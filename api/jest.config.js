module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'node',
  roots: ['<rootDir>/src', '<rootDir>/src/__tests__'],
  testMatch: ['**/__tests__/**/*.ts', '**/?(*.)+(spec|test).ts'],
  setupFiles: ['<rootDir>/jest.setup.ts'],
  transform: {
    '^.+\\.ts$': 'ts-jest',
  },
  collectCoverageFrom: [
    'src/**/*.ts',
    '!src/**/*.test.ts',
    '!src/**/*.spec.ts',
    '!src/index.ts',
  ],
  coverageThreshold: {
    global: {
      branches: 54,
      // The project currently includes runtime code that isn't exercised by unit tests
      // (e.g. websocket server and auth helpers). Keep thresholds realistic so
      // CI focuses on regressions rather than failing the gate for missing coverage.
      functions: 35,
      lines: 60,
      statements: 60,
    },
  },
  coverageDirectory: 'coverage',
  verbose: true,
};
