/** @type {import('@stryker-mutator/api/core').PartialStrykerOptions} */
export default {
  mutate: ['src/utils/pagination.ts'],
  testRunner: 'jest',
  coverageAnalysis: 'perTest',
  jest: {
    projectType: 'custom',
    configFile: 'jest.mutation.config.js',
    enableFindRelatedTests: true,
  },
  reporters: ['clear-text', 'html', 'json'],
  htmlReporter: {
    fileName: 'reports/mutation/index.html',
  },
  jsonReporter: {
    fileName: 'reports/mutation/report.json',
  },
  tempDirName: '.stryker-tmp',
  concurrency: 2,
  thresholds: {
    high: 80,
    low: 70,
    break: 70,
  },
};