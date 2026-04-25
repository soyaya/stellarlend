import { Router } from 'express';
import * as gasController from '../controllers/gas.controller';
import { prepareValidation } from '../middleware/validation';

const router: Router = Router();

/**
 * @openapi
 * /gas/estimate/{operation}:
 *   get:
 *     summary: Estimate gas for a lending transaction
 *     description: Simulates the transaction on the Stellar network to estimate CPU, memory, and fee requirements.
 *     tags:
 *       - Gas
 *     parameters:
 *       - in: path
 *         name: operation
 *         required: true
 *         schema:
 *           type: string
 *           enum: [deposit, borrow, repay, withdraw]
 *       - in: query
 *         name: userAddress
 *         required: true
 *         schema:
 *           type: string
 *       - in: query
 *         name: amount
 *         required: true
 *         schema:
 *           type: string
 *       - in: query
 *         name: assetAddress
 *         required: false
 *         schema:
 *           type: string
 *     responses:
 *       200:
 *         description: Gas estimate successful
 *       400:
 *         description: Validation error
 *       500:
 *         description: Internal server error
 */
router.get('/estimate/:operation', prepareValidation, gasController.estimateGas);

export default router;
