import { Router } from 'express';
import * as verificationController from '../controllers/verification.controller';

const router: Router = Router();

/**
 * @openapi
 * /verification:
 *   get:
 *     summary: Verify contract against source code
 *     tags:
 *       - Verification
 *     parameters:
 *       - name: contractId
 *         in: query
 *         required: true
 *         schema:
 *           type: string
 *         description: Contract ID to verify
 *       - name: network
 *         in: query
 *         schema:
 *           type: string
 *           default: testnet
 *         description: Network to verify on
 *     responses:
 *       200:
 *         description: Verification result
 *         content:
 *           application/json:
 *             schema:
 *               type: object
 *               properties:
 *                 verified:
 *                   type: boolean
 *                 contractId:
 *                   type: string
 *                 network:
 *                   type: string
 *                 message:
 *                   type: string
 *       400:
 *         description: Bad request
 *       500:
 *         description: Verification error
 */
router.get('/', verificationController.verifyContract);

export default router;