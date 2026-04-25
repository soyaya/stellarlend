import { Request, Response, NextFunction } from 'express';
import { StellarService } from '../services/stellar.service';
import { LendingOperation } from '../types';

const stellarService = new StellarService();

export const estimateGas = async (
  req: Request,
  res: Response,
  next: NextFunction
): Promise<void> => {
  try {
    const { operation } = req.params;
    const { userAddress, amount, assetAddress } = req.query as {
      userAddress: string;
      amount: string;
      assetAddress?: string;
    };

    const gasEstimate = await stellarService.estimateGas(
      operation as LendingOperation,
      userAddress,
      assetAddress,
      amount
    );

    res.json({
      success: true,
      data: gasEstimate,
    });
  } catch (error) {
    next(error);
  }
};
