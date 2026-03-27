import { Router } from 'express';
import * as lendingController from '../controllers/lending.controller';

const router = Router();

/**
 * @openapi
 * /health:
 *   get:
 *     summary: Check API and service health
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
