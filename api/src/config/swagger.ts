import swaggerJsdoc from 'swagger-jsdoc';

const options: swaggerJsdoc.Options = {
  definition: {
    openapi: '3.0.3',
    info: {
      title: 'StellarLend API',
      version: '1.0.0',
      description: 'REST API for StellarLend core lending operations on Stellar/Soroban',
      license: {
        name: 'MIT',
      },
    },
    servers: [
      {
        url: '/api',
        description: 'API base path',
      },
    ],
    components: {
      schemas: {
        PrepareResponse: {
          type: 'object',
          properties: {
            unsignedXdr: { type: 'string', description: 'Unsigned transaction XDR' },
            operation: {
              type: 'string',
              enum: ['deposit', 'borrow', 'repay', 'withdraw'],
            },
            expiresAt: { type: 'string', format: 'date-time', description: 'XDR expiration timestamp' },
          },
          required: ['unsignedXdr', 'operation', 'expiresAt'],
        },
        SubmitRequest: {
          type: 'object',
          properties: {
            signedXdr: { type: 'string', description: 'Signed transaction XDR' },
          },
          required: ['signedXdr'],
        },
        TransactionResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean' },
            transactionHash: { type: 'string' },
            status: {
              type: 'string',
              enum: ['pending', 'success', 'failed', 'cancelled'],
            },
            message: { type: 'string' },
            error: { type: 'string' },
            ledger: { type: 'integer' },
            details: { description: 'Optional raw provider payload for debugging' },
          },
          required: ['success', 'status'],
        },
        HealthCheckResponse: {
          type: 'object',
          properties: {
            status: { type: 'string', enum: ['healthy', 'unhealthy'] },
            timestamp: { type: 'string', format: 'date-time' },
            services: {
              type: 'object',
              properties: {
                horizon: { type: 'boolean' },
                sorobanRpc: { type: 'boolean' },
              },
              required: ['horizon', 'sorobanRpc'],
            },
          },
          required: ['status', 'timestamp', 'services'],
        },
        ErrorResponse: {
          type: 'object',
          properties: {
            success: { type: 'boolean', example: false },
            error: { type: 'string' },
          },
          required: ['success', 'error'],
        },
      },
    },
  },
  apis: ['./src/routes/*.ts'],
};

export const swaggerSpec = swaggerJsdoc(options);
