import { Router } from 'express';
import * as lendingController from '../controllers/lending.controller';
import { requireRole } from '../middleware/rbac';

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
router.get('/pause-status', requireRole('operator'), lendingController.getPauseStatus);
router.post('/pause', requireRole('admin'), lendingController.setManualPause);
router.post('/resume', requireRole('admin'), lendingController.resumeProtocol);
router.get('/roles', requireRole('operator'), lendingController.listRoleAssignments);
router.post('/roles/assign', requireRole('admin'), lendingController.assignAccessRole);
router.post('/roles/revoke', requireRole('admin'), lendingController.revokeAccessRole);

/**
 * @openapi
 * /protocol/audit-logs:
 *   get:
 *     summary: Search audit logs
 *     description: >
 *       Returns structured audit log entries. Supports filtering by action, actor, status,
 *       and date range. All state changes (transactions, role assignments, pause/resume)
 *       are captured with before/after values and a SHA-256 integrity hash chain.
 *     tags:
 *       - Protocol
 *     parameters:
 *       - { in: query, name: action,  schema: { type: string }, description: "Filter by action (e.g. DEPOSIT)" }
 *       - { in: query, name: actor,   schema: { type: string }, description: "Filter by actor address" }
 *       - { in: query, name: status,  schema: { type: string }, description: "Filter by status" }
 *       - { in: query, name: from,    schema: { type: string, format: date-time } }
 *       - { in: query, name: to,      schema: { type: string, format: date-time } }
 *       - { in: query, name: limit,   schema: { type: integer, default: 100 } }
 *       - { in: query, name: offset,  schema: { type: integer, default: 0 } }
 */
router.get('/audit-logs', requireRole('operator'), lendingController.getAuditLogs);

/**
 * @openapi
 * /protocol/audit-logs/export:
 *   get:
 *     summary: Export audit logs as JSON
 *     description: Downloads the filtered audit log as a JSON attachment for auditors.
 *     tags:
 *       - Protocol
 */
router.get('/audit-logs/export', requireRole('operator'), lendingController.exportAuditLogs);

/**
 * @openapi
 * /protocol/audit-logs/verify:
 *   get:
 *     summary: Verify audit log integrity
 *     description: >
 *       Recalculates and verifies the SHA-256 hash chain across all stored audit entries.
 *       Returns 200 when intact, 409 when tampering is detected.
 *     tags:
 *       - Protocol
 */
router.get('/audit-logs/verify', requireRole('operator'), lendingController.verifyAuditLogIntegrity);

export default router;
