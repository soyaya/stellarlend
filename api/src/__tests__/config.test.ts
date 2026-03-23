describe('Config Validation', () => {
  const originalEnv = process.env;

  beforeEach(() => {
    jest.resetModules();
    process.env = { ...originalEnv };
  });

  afterAll(() => {
    process.env = originalEnv;
  });

  it('should throw an error if CONTRACT_ID is missing', () => {
    delete process.env.CONTRACT_ID;

    expect(() => {
      require('../config/index');
    }).toThrow('CONTRACT_ID environment variable is required');
  });

  it('should not throw an error if CONTRACT_ID is present', () => {
    process.env.CONTRACT_ID = 'TEST_CONTRACT_ID';

    expect(() => {
      require('../config/index');
    }).not.toThrow();
  });
});
