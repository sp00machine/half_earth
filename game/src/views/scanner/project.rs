use std::collections::BTreeMap;

use hes_engine::{Flag, Id, Project, ProjectType};

use crate::{
    consts,
    state::{GameState, PlanChange, StateExt, Tutorial},
    views::scanner::ScanResult,
};

use super::Scannable;

fn is_refundable(project: &Project, plan_changes: &BTreeMap<Id, PlanChange>) -> bool {
    if let Some(changes) = plan_changes.get(&project.id) {
        changes.upgrades > 0 || changes.points > 0 || changes.passed || changes.withdrawn
    } else {
        false
    }
}

fn is_subtractable(project: &Project, plan_changes: &BTreeMap<Id, PlanChange>) -> bool {
    project.is_haltable() || is_refundable(project, plan_changes)
}

impl Scannable for Project {
    fn add_scan_time(&self) -> f32 {
        consts::PROJECT_CARD_SCAN_TIME
    }

    fn add_scan_done(&self, state: &mut GameState) -> ScanResult {
        // The scanner holds a copy of the card from when the list was
        // opened; use the live project (e.g. a policy passed a moment ago).
        let live = state.core.world.projects[&self.id].clone();
        let this = &live;
        let result;
        let changes = state.ui.plan_changes.entry(this.id).or_default();

        // Upgrading projects
        if this.is_passed_or_online() && this.next_upgrade().is_some() {
            let free = changes.downgrades > 0;
            if free {
                changes.downgrades -= 1;
            }
            state
                .core
                .upgrade_project_ext(&this.id, free, &mut state.ui.queued_upgrades);

            if state.core.world.projects[&this.id].next_upgrade().is_some() {
                result = ScanResult::SuccessContinue;
            } else {
                result = ScanResult::SuccessStop;
            }

            // Refundable upgrade
            changes.upgrades += 1;

        // Adding points to Research/Infrastructure
        } else if this.kind != ProjectType::Policy
            && state.core.buy_point(&this.id, &mut state.ui.points)
        {
            if state.ui.tutorial == Tutorial::Projects {
                state.ui.tutorial.advance();
            }
            state.core.assign_point(&this.id, &mut state.ui.points);

            // Refundable points
            changes.points += 1;

            result = ScanResult::SuccessContinue;

        // Passing Policies
        // Free if withdrawn in this same session (i.e. undo the withdraw)
        } else if this.kind == ProjectType::Policy
            && (changes.withdrawn || state.core.pay_points(&this.id))
        {
            if state.ui.tutorial == Tutorial::Projects {
                state.ui.tutorial.advance();
            }
            state.core.pass_policy(&this.id);

            // Refundable
            if changes.withdrawn {
                changes.withdrawn = false;
            } else {
                changes.passed = true;
            }

            if state.core.world.projects[&this.id].next_upgrade().is_some() {
                result = ScanResult::SuccessContinue;
            } else {
                result = ScanResult::SuccessStop;
            }

        // If not enough PC
        } else {
            result = ScanResult::Rejected;
        }

        result
    }

    fn is_add_allowed(&self, state: &GameState) -> bool {
        let this = &state.core.world.projects[&self.id];
        let player_seats = state.npcs.coalition_seats();
        let parliament_suspended = state.flags.contains(&Flag::ParliamentSuspended);

        // Does this project already have an upgrade queued/under construction?
        let upgrade_queued = state.ui.queued_upgrades.get(&this.id) == Some(&true);

        // Points can be added to a project if:
        // - The player has enough of a majority, if required.
        // - No upgrade is queued for the project.
        // - An upgrade exists for the project.
        // - If the project is a policy, only if it's not already implemented.
        #[allow(clippy::if_same_then_else)]
        #[allow(clippy::needless_bool)]
        // The majority needed: to pass the project, or, once it's in
        // force, to raise it to its next level.
        let required_majority = if this.is_passed_or_online() {
            this.next_upgrade().map_or(0., |u| u.required_majority)
        } else {
            this.required_majority
        };
        if (required_majority > 0. && player_seats < required_majority) && !parliament_suspended {
            false
        } else if upgrade_queued {
            false
        } else if this.next_upgrade().is_some() {
            true
        } else if this.is_passed_or_online() {
            // Already in force or passed this session (a policy, a finished project, or an ongoing
            // one like Direct Air Capture) with no level left to add:
            // points would do nothing.
            false
        } else {
            true
        }
    }

    fn rem_scan_time(&self) -> f32 {
        consts::PROJECT_CARD_WITHDRAW_TIME
    }

    fn rem_scan_done(&self, state: &mut GameState) -> ScanResult {
        let live = state.core.world.projects[&self.id].clone();
        let this = &live;
        let mut keep_withdrawing = false;
        let refundable = is_refundable(self, &state.ui.plan_changes);
        let changes = state.ui.plan_changes.entry(this.id).or_default();
        if refundable {
            if changes.upgrades > 0 {
                state
                    .core
                    .downgrade_project_ext(&this.id, &mut state.ui.queued_upgrades);
                changes.upgrades -= 1;
                keep_withdrawing = changes.upgrades > 0 || changes.passed;
            } else if changes.passed {
                state.core.stop_policy(&this.id);
                changes.passed = false;
            } else {
                let points = changes.points;
                let mut refund = state.core.next_point_cost(&this.kind) * points;

                // Don't allow stored research-only points to be converted into PC,
                // instead convert them back into research points
                if this.kind == ProjectType::Research {
                    let n_refundable_points = state.ui.points.refundable_research.len();
                    let excess_points = points.saturating_sub(n_refundable_points);
                    let n_refunded_points = points - excess_points;
                    refund = state
                        .ui
                        .points
                        .refundable_research
                        .refund(n_refunded_points);
                    state.ui.points.research += excess_points as isize;
                }
                state.core.unassign_points(&this.id, points);
                state.core.change_political_capital(refund as isize);
                changes.points = 0;
            }
        } else if this.can_downgrade() {
            state.core.downgrade_project(&this.id);
            keep_withdrawing = this.level > 0;
            changes.downgrades += 1;
        } else {
            state.core.stop_project(&this.id);
            changes.withdrawn = true;
        }

        if keep_withdrawing {
            ScanResult::SuccessContinue
        } else {
            ScanResult::SuccessStop
        }
    }

    fn is_rem_allowed(&self, state: &GameState) -> bool {
        is_subtractable(&state.core.world.projects[&self.id], &state.ui.plan_changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scanner keeps a copy of each card from when the list opened. After
    /// passing a policy, scanning that same copy again must raise it a level,
    /// not try to pass it again, and it can go up several levels in one go.
    #[test]
    fn a_policy_can_be_passed_and_raised_from_the_same_card() {
        let mut state = GameState::default();
        state.core.political_capital = 500;
        let card = state
            .core
            .world
            .projects
            .iter()
            .find(|p| p.name == "Plant-Based Diets")
            .unwrap()
            .clone();
        let id = card.id;

        assert!(card.is_add_allowed(&state));
        assert!(matches!(
            card.add_scan_done(&mut state),
            ScanResult::SuccessContinue
        ));
        assert_eq!(
            state.core.world.projects[&id].display_name(),
            "Diet: Meatless Mondays"
        );

        // Flexitarian needs no majority.
        assert!(card.is_add_allowed(&state));
        assert!(matches!(
            card.add_scan_done(&mut state),
            ScanResult::SuccessContinue
        ));
        assert_eq!(
            state.core.world.projects[&id].display_name(),
            "Diet: Flexitarian"
        );

        // Vegetarian needs a majority the player doesn't have at the start.
        if state.npcs.coalition_seats() < 0.5 {
            assert!(!card.is_add_allowed(&state));
            state.core.flags.push(Flag::ParliamentSuspended);
        }
        assert!(card.is_add_allowed(&state));
        card.add_scan_done(&mut state);
        card.add_scan_done(&mut state);
        assert_eq!(
            state.core.world.projects[&id].display_name(),
            "Diet: Vegan Mandate"
        );
        assert!(!card.is_add_allowed(&state));

        // And back down, one level per scan, then withdrawn.
        for _ in 0..4 {
            assert!(card.is_rem_allowed(&state));
            card.rem_scan_done(&mut state);
        }
        assert_eq!(state.core.world.projects[&id].level, 0);
        assert!(!state.core.world.projects[&id].is_passed_or_online());
    }
}
