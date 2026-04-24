import { Request, Response, NextFunction } from 'express';
import { randomUUID } from 'crypto';
import { requestContext } from '../utils/requestContext';

export const requestIdMiddleware = (req: Request, res: Response, next: NextFunction) => {
  const reqId = req.headers['x-request-id'];
  req.id = (Array.isArray(reqId) ? reqId[0] : reqId) || randomUUID();
  res.setHeader('x-request-id', req.id);
  
  // Set the request ID in the async local storage context for logger propagation
  requestContext.run(req.id, () => {
    next();
  });
};
