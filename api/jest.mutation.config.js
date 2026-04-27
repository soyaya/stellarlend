const baseConfig = require('./jest.config.js');

module.exports = {
  ...baseConfig,
  testMatch: ['**/pagination.test.ts'],
  collectCoverage: false,
  coverageThreshold: undefined,
};