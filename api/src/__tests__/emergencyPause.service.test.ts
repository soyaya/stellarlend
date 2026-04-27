import { emergencyPauseService } from '../services/emergencyPause.service';

describe('EmergencyPauseService', () => {
  beforeEach(() => {
    emergencyPauseService.resume();
    emergencyPauseService.drainWithdrawalQueue();
  });

  it('queues withdrawals while paused', () => {
    emergencyPauseService.pause('manual');
    emergencyPauseService.queueWithdrawal({
      userAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF',
      amount: '10',
    });

    expect(emergencyPauseService.getWithdrawalQueue()).toHaveLength(1);
  });

  it('drains the queued withdrawals on resume flow', () => {
    emergencyPauseService.pause('manual');
    emergencyPauseService.queueWithdrawal({
      userAddress: 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF',
      amount: '10',
    });
    const drained = emergencyPauseService.drainWithdrawalQueue();
    emergencyPauseService.resume();

    expect(drained).toHaveLength(1);
    expect(emergencyPauseService.isPaused().paused).toBe(false);
  });
});
