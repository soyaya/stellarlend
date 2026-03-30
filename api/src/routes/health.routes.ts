import { Router } from 'express';
import * as lendingController from '../controllers/lending.controller';

const router: Router = Router();

/**
 * @openapi
 * /health/live:
 *   get:
 *     summary: Fast liveness probe
 *     tags:
 *       - Health
 *     responses:
 *       200:
 *         description: API process is alive
 *         content:
 *           application/json:
 *             schema:
 *               type: object
 *               properties:
 *                 status:
 *                   type: string
 *                   example: ok
 */
router.get('/live', lendingController.livenessCheck);

/**
 * @openapi
 * /health/ready:
 *   get:
 *     summary: Readiness probe with upstream dependency checks
 *     tags:
 *       - Health
 *     responses:
 *       200:
 *         description: Dependencies are ready
 *         content:
 *           application/json:
 *             schema:
 *               type: object
 *               properties:
 *                 status:
 *                   type: string
 *                   example: ok
 *                 horizon:
 *                   type: string
 *                   example: up
 *                 soroban:
 *                   type: string
 *                   example: up
 *       503:
 *         description: One or more dependencies are unavailable
 */
router.get('/ready', lendingController.readinessCheck);

/**
 * @openapi
 * /health:
 *   get:
 *     summary: Backward-compatible readiness summary
 *     tags:
 *       - Health
 *     responses:
 *       200:
 *         description: All services are healthy
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/HealthCheckResponse'
 *       503:
 *         description: One or more services are unhealthy
 *         content:
 *           application/json:
 *             schema:
 *               $ref: '#/components/schemas/HealthCheckResponse'
 */
router.get('/', lendingController.healthCheck);

export default router;
