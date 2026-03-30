import { Router } from 'express';
import * as lendingController from '../controllers/lending.controller';

const router: Router = Router();

/**
 * @openapi
 * /protocol/stats:
 *   get:
 *     summary: Get protocol-level statistics
 *     description: Returns cached protocol analytics sourced from the smart contract state.
 *     tags:
 *       - Protocol
 *     responses:
 *       200:
 *         description: Protocol statistics
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ProtocolStatsResponse'
 *       500:
 *         description: Internal server error
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/ErrorResponse'
 */
router.get('/stats', lendingController.protocolStats);

export default router;
