import { RequestCoalescingService } from '../services/requestCoalescing.service';

describe('RequestCoalescingService', () => {
  let service: RequestCoalescingService;

  beforeEach(() => {
    service = new RequestCoalescingService();
  });

  describe('execute', () => {
    it('should coalesce concurrent identical requests', async () => {
      const mockOperation = jest.fn().mockResolvedValue('result');
      const key = 'test-key';

      // Start multiple concurrent requests
      const promises = [
        service.execute(key, mockOperation, 5000),
        service.execute(key, mockOperation, 5000),
        service.execute(key, mockOperation, 5000),
      ];

      const results = await Promise.all(promises);

      // All should return the same result
      expect(results).toEqual(['result', 'result', 'result']);
      // Operation should only be called once
      expect(mockOperation).toHaveBeenCalledTimes(1);
    });

    it('should handle different keys separately', async () => {
      const mockOperation1 = jest.fn().mockResolvedValue('result1');
      const mockOperation2 = jest.fn().mockResolvedValue('result2');

      const promises = [
        service.execute('key1', mockOperation1, 5000),
        service.execute('key2', mockOperation2, 5000),
      ];

      const results = await Promise.all(promises);

      expect(results).toEqual(['result1', 'result2']);
      expect(mockOperation1).toHaveBeenCalledTimes(1);
      expect(mockOperation2).toHaveBeenCalledTimes(1);
    });

    it('should timeout if operation takes too long', async () => {
      const mockOperation = jest.fn().mockImplementation(
        () => new Promise(resolve => setTimeout(() => resolve('result'), 100))
      );

      const promise = service.execute('key', mockOperation, 50); // 50ms timeout

      await expect(promise).rejects.toThrow('Request coalescing timeout');
    });
  });

  describe('getStats', () => {
    it('should return coalescing statistics', async () => {
      const mockOperation = jest.fn().mockResolvedValue('result');

      // Execute some coalesced requests
      await Promise.all([
        service.execute('key1', mockOperation, 5000),
        service.execute('key1', mockOperation, 5000),
        service.execute('key2', mockOperation, 5000),
      ]);

      const stats = service.getStats();

      expect(stats).toHaveProperty('totalRequests');
      expect(stats).toHaveProperty('coalescedRequests');
      expect(stats).toHaveProperty('activeOperations');
      expect(stats.totalRequests).toBe(3);
      expect(stats.coalescedRequests).toBe(1); // One coalesced request for key1
    });
  });
});