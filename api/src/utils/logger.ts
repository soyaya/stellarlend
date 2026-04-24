import winston from 'winston';
import { config } from '../config';
import { requestContext } from './requestContext';

const addRequestId = winston.format((info: winston.Logform.TransformableInfo) => {
  const reqId = requestContext.getStore();
  if (reqId) {
    info.requestId = reqId;
  }
  return info;
});

const logger = winston.createLogger({
  level: config.logging.level,
  format: winston.format.combine(
    addRequestId(),
    winston.format.timestamp(),
    winston.format.errors({ stack: true }),
    winston.format.json()
  ),
  transports: [
    new winston.transports.Console({
      format: winston.format.combine(winston.format.colorize(), winston.format.simple()),
    }),
  ],
});

export default logger;
