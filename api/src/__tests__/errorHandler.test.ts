import { Request, Response, NextFunction } from 'express';
import { errorHandler } from '../middleware/errorHandler';
import {
  ValidationError,
  UnauthorizedError,
  NotFoundError,
  ConflictError,
  InternalServerError,
} from '../utils/errors';

describe('Error Handler Middleware', () => {
  let mockRequest: Partial<Request>;
  let mockResponse: Partial<Response>;
  let mockNext: NextFunction;

  beforeEach(() => {
    mockRequest = {
      path: '/api/test',
      method: 'POST',
    };
    mockResponse = {
      status: jest.fn().mockReturnThis(),
      json: jest.fn().mockReturnThis(),
    };
    mockNext = jest.fn();
  });

  it('should handle ValidationError with correct status code and error code', () => {
    const error = new ValidationError('Invalid input');

    errorHandler(error, mockRequest as Request, mockResponse as Response, mockNext);

    expect(mockResponse.status).toHaveBeenCalledWith(400);
    expect(mockResponse.json).toHaveBeenCalledWith({
      success: false,
      error: 'Invalid input',
      code: 'VALIDATION_ERROR',
    });
  });

  it('should handle UnauthorizedError', () => {
    const error = new UnauthorizedError();

    errorHandler(error, mockRequest as Request, mockResponse as Response, mockNext);

    expect(mockResponse.status).toHaveBeenCalledWith(401);
    expect(mockResponse.json).toHaveBeenCalledWith({
      success: false,
      error: 'Unauthorized',
      code: 'UNAUTHORIZED',
    });
  });

  it('should handle NotFoundError with correct status code and error code', () => {
    const error = new NotFoundError('Resource not found');

    errorHandler(error, mockRequest as Request, mockResponse as Response, mockNext);

    expect(mockResponse.status).toHaveBeenCalledWith(404);
    expect(mockResponse.json).toHaveBeenCalledWith({
      success: false,
      error: 'Resource not found',
      code: 'NOT_FOUND',
    });
  });

  it('should handle ConflictError with correct status code and error code', () => {
    const error = new ConflictError('Resource already exists');

    errorHandler(error, mockRequest as Request, mockResponse as Response, mockNext);

    expect(mockResponse.status).toHaveBeenCalledWith(409);
    expect(mockResponse.json).toHaveBeenCalledWith({
      success: false,
      error: 'Resource already exists',
      code: 'CONFLICT',
    });
  });

  it('should handle InternalServerError with correct status code and error code', () => {
    const error = new InternalServerError('Something went wrong');

    errorHandler(error, mockRequest as Request, mockResponse as Response, mockNext);

    expect(mockResponse.status).toHaveBeenCalledWith(500);
    expect(mockResponse.json).toHaveBeenCalledWith({
      success: false,
      error: 'Something went wrong',
      code: 'INTERNAL_SERVER_ERROR',
    });
  });

  it('should handle generic errors with 500 status and INTERNAL_SERVER_ERROR code', () => {
    const error = new Error('Something went wrong');

    errorHandler(error, mockRequest as Request, mockResponse as Response, mockNext);

    expect(mockResponse.status).toHaveBeenCalledWith(500);
    expect(mockResponse.json).toHaveBeenCalledWith({
      success: false,
      error: 'Internal server error',
      code: 'INTERNAL_SERVER_ERROR',
    });
  });

  it('should handle SyntaxError with 400 status and VALIDATION_ERROR code', () => {
    const error = new SyntaxError('Invalid JSON');

    errorHandler(error, mockRequest as Request, mockResponse as Response, mockNext);

    expect(mockResponse.status).toHaveBeenCalledWith(400);
    expect(mockResponse.json).toHaveBeenCalledWith({
      success: false,
      error: 'Invalid JSON',
      code: 'VALIDATION_ERROR',
    });
  });
});
