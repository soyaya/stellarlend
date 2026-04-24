import { assignRole, getCurrentRoleAssignments, scheduleRevocation } from '../middleware/rbac';

describe('RBAC hierarchy', () => {
  it('allows admin to assign operator role', () => {
    assignRole('admin', 'GTESTOPERATOR', 'operator');
    expect(getCurrentRoleAssignments()['GTESTOPERATOR']).toBe('operator');
  });

  it('requires cool-off for revocation', () => {
    assignRole('admin', 'GTESTADMIN', 'operator');
    scheduleRevocation('GADMIN', 'admin', 'GTESTADMIN', 'operator', 1);
    expect(getCurrentRoleAssignments()['GTESTADMIN']).toBe('operator');
  });
});
