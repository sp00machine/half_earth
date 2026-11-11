import initHector from './init';

// Base emissions scenario for Hector
// Paths are relative to the page's base URL so the game
// also works when hosted under a subpath (e.g. GitHub Pages).
const baseEmissionsScenario = 'hector/rcp26.to_2050.json';
const defaultEmissionsScenario = 'hector/rcp26.default_emissions.json';
const START_YEAR = 1765; // Should match `baseEmissionsScenario["startYear"]`

const hectorOutputVars = {
  'temperature.Tgav': {
    'component': 'temperature',
    'description': 'global atmospheric temperature anomaly',
    'unit': 'degC',
    'variable': 'Tgav'
  }
};

class Temperature {
  constructor(startYear) {
    this.startYear = startYear;

    this.emissions = {
      startYear: START_YEAR,
      data: {},
    };

    // Emissions added or restored before the data files below finish
    // loading are held here and applied once loading is done.
    this.restored = null;
    this.pending = [];

    const getJSON = (url) => fetch(url).then((resp) => resp.json());
    this.loaded = Promise.all([
      getJSON('hector/config.json'),
      getJSON(defaultEmissionsScenario),
      getJSON(baseEmissionsScenario),
    ]).then(([config, defaultEmissions, baseScenario]) => {
      this.config = config;
      this.defaultEmissions = defaultEmissions;

      if (this.restored) {
        // A saved emissions history (from earlier planning cycles) was
        // restored while we were loading. Keep it: replacing it with the
        // base scenario would erase every year of the player's emissions.
        this.emissions.data = this.restored;
      } else {
        this.emissions = {
          startYear: baseScenario['startYear'],
          data: {}
        };

        // Only get the base scenario data up to the game starting year
        let baseYears = this.startYear - baseScenario['startYear'];
        Object.keys(baseScenario['data']).forEach((k) => {
          this.emissions['data'][k] = baseScenario['data'][k].slice(0, baseYears);
        });
      }

      this.pending.forEach((emissions) => this._pushEmissions(emissions));
      this.pending = [];
    });
  }

  /*
   * Adds a year of emissions data.
   * `emissions` should have keys and values for each required
   * emissions type. Refer to `/assets/hector/rcp26.to_2050.json`
   * for the required keys.
   */
  addEmissions(emissions) {
    if (!this.defaultEmissions) {
      this.pending.push(emissions);
      return;
    }
    this._pushEmissions(emissions);
  }

  _pushEmissions(emissions) {
    Object.keys(this.defaultEmissions).forEach((k) => {
      let val = emissions[k] !== undefined ? emissions[k] : this.defaultEmissions[k];
      this.emissions['data'][k].push(val);
    });
  }

  setEmissions(emissions) {
    this.emissions.data = emissions;
    this.restored = emissions;
  }

  getEmissions() {
    return this.emissions.data;
  }

  updateTemperature() {
    let ready = this.loaded.then(() => this._hector ?
      this._hector :
      initHector(this.config, hectorOutputVars).then((hector) => {
        this._hector = hector;
      }));

    return ready.then(() => {
      // Calculate new avg global temp
      // Only compute up to the current year,
      // so the last returned tgav is the current tgav
      let endDate = this.emissions.startYear + this.emissions.data['ffi_emissions'].length;
      let results = this._hector.run(endDate, this.emissions);
      let avgGlobalTemps = results['temperature.Tgav'];
      let avgGlobalTemp = avgGlobalTemps[avgGlobalTemps.length - 1];
      return avgGlobalTemp;
    });
  }
}

export { Temperature };
