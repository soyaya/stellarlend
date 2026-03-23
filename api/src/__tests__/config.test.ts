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

  it('should not throw an error if CONTRACT_ID and secure JWT_SECRET are present', () => {
    process.env.CONTRACT_ID = 'TEST_CONTRACT_ID';
    process.env.JWT_SECRET = 'a-secure-secret-that-is-at-least-thirty-two-characters-long';

    expect(() => {
      require('../config/index');
    }).not.toThrow();
  });

  it('should throw an error if JWT_SECRET is missing', () => {
    process.env.CONTRACT_ID = 'TEST_CONTRACT_ID';
    delete process.env.JWT_SECRET;

    expect(() => {
      require('../config/index');
    }).toThrow('JWT_SECRET must be set to a strong secret (min 32 chars)');
  });

  it('should throw an error if JWT_SECRET is the default insecure value', () => {
    process.env.CONTRACT_ID = 'TEST_CONTRACT_ID';
    process.env.JWT_SECRET = 'default-secret-change-me';

    expect(() => {
      require('../config/index');
    }).toThrow('JWT_SECRET must be set to a strong secret (min 32 chars)');
  });

  it('should throw an error if JWT_SECRET is too short', () => {
    process.env.CONTRACT_ID = 'TEST_CONTRACT_ID';
    process.env.JWT_SECRET = 'too-short';

    expect(() => {
      require('../config/index');
    }).toThrow('JWT_SECRET must be set to a strong secret (min 32 chars)');
  });
});
