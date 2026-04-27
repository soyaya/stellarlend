import { NextFunction, Request, Response } from 'express';
import { UnauthorizedError, ValidationError } from '../utils/errors';

export type Role = 'admin' | 'operator' | 'user';

const ROLE_WEIGHT: Record<Role, number> = {
  admin: 3,
  operator: 2,
  user: 1,
};

type PendingRevocation = {
  role: Role;
  actor: string;
  effectiveAt: number;
};

const pendingRevocations = new Map<string, PendingRevocation>();
const currentRoles = new Map<string, Role>();

function resolveRole(req: Request): Role {
  const role = (req.headers['x-user-role'] || 'user').toString().toLowerCase();
  if (role === 'admin' || role === 'operator' || role === 'user') return role;
  throw new ValidationError('x-user-role must be one of: admin, operator, user');
}

function resolveActor(req: Request): string {
  const actor = (req.headers['x-user-address'] || '').toString().trim();
  if (!actor) throw new UnauthorizedError('x-user-address header is required');
  return actor;
}

export function requireRole(minimum: Role) {
  return (req: Request, _res: Response, next: NextFunction) => {
    const callerRole = resolveRole(req);
    if (ROLE_WEIGHT[callerRole] < ROLE_WEIGHT[minimum]) {
      throw new UnauthorizedError(`Role ${minimum} or higher required`);
    }
    next();
  };
}

export function scheduleRoleRevocation(
  actor: string,
  role: Role,
  target: string,
  coolOffMs: number
): void {
  if (ROLE_WEIGHT[role] >= ROLE_WEIGHT.admin && actor === target) {
    throw new ValidationError('admin self-revocation is not allowed');
  }
  pendingRevocations.set(target, {
    role,
    actor,
    effectiveAt: Date.now() + coolOffMs,
  });
}

export function applyMatureRoleRevocations(currentRoles: Map<string, Role>): void {
  const now = Date.now();
  for (const [target, revocation] of pendingRevocations.entries()) {
    if (revocation.effectiveAt <= now) {
      const current = currentRoles.get(target);
      if (current === revocation.role) currentRoles.set(target, 'user');
      pendingRevocations.delete(target);
    }
  }
}

export function assignRole(actorRole: Role, target: string, targetRole: Role): void {
  if (ROLE_WEIGHT[actorRole] <= ROLE_WEIGHT[targetRole]) {
    throw new UnauthorizedError('Role assignment requires a higher role than target');
  }
  currentRoles.set(target, targetRole);
}

export function scheduleRevocation(
  actor: string,
  actorRole: Role,
  target: string,
  role: Role,
  coolOffMs: number
): void {
  if (ROLE_WEIGHT[actorRole] <= ROLE_WEIGHT[role]) {
    throw new UnauthorizedError('Role revocation requires a higher role than target');
  }
  scheduleRoleRevocation(actor, role, target, coolOffMs);
}

export function getCurrentRoleAssignments(): Record<string, Role> {
  applyMatureRoleRevocations(currentRoles);
  return Object.fromEntries(currentRoles.entries());
}

export function roleAssignmentAllowed(req: Request, targetRole: Role): boolean {
  const callerRole = resolveRole(req);
  return ROLE_WEIGHT[callerRole] > ROLE_WEIGHT[targetRole];
}

export function getRbacAuditContext(req: Request): { actor: string; role: Role } {
  return {
    actor: resolveActor(req),
    role: resolveRole(req),
  };
}
