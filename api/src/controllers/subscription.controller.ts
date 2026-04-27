import { NextFunction, Request, Response } from 'express';
import { ValidationError } from '../utils/errors';
import { SubscriptionService } from '../services/subscription.service';

const subscriptionService = new SubscriptionService();

function normalizeImportError(error: unknown): Error {
  if (error instanceof ValidationError) {
    return error;
  }

  if (error instanceof Error) {
    return new ValidationError(error.message);
  }

  return new ValidationError('Invalid import request');
}

export const validateImportRequest = (req: Request, res: Response, next: NextFunction) => {
  try {
    const result = subscriptionService.validateImport(req.body);
    return res.status(200).json(result);
  } catch (error) {
    return next(normalizeImportError(error));
  }
};

export const previewImportRequest = (req: Request, res: Response, next: NextFunction) => {
  try {
    const result = subscriptionService.previewImport(req.body);
    return res.status(200).json(result);
  } catch (error) {
    return next(normalizeImportError(error));
  }
};

export const importSubscriptionsRequest = (req: Request, res: Response, next: NextFunction) => {
  try {
    const result = subscriptionService.importSubscriptions(req.body);
    if (result.errorCount > 0) {
      return res.status(400).json({
        success: false,
        ...result,
      });
    }

    return res.status(200).json({
      success: true,
      ...result,
    });
  } catch (error) {
    return next(normalizeImportError(error));
  }
};

export const exportSubscriptionsRequest = (req: Request, res: Response, next: NextFunction) => {
  try {
    const result = subscriptionService.exportSubscriptions(req.params.merchantId);
    return res.status(200).json(result);
  } catch (error) {
    return next(error);
  }
};

export const getImportHistoryRequest = (req: Request, res: Response, next: NextFunction) => {
  try {
    const history = subscriptionService.getImportHistory(req.params.merchantId);
    return res.status(200).json({
      merchantId: req.params.merchantId,
      history,
    });
  } catch (error) {
    return next(error);
  }
};
