import { Router } from 'express';
import * as stakingController from '../controllers/staking.controller';

const router: Router = Router();

router.post('/stake', stakingController.stake);
router.post('/unstake', stakingController.unstake);
router.post('/delegate', stakingController.delegate);
router.post('/revoke-delegation', stakingController.revokeDelegation);
router.post('/claim-rewards/:userAddress', stakingController.claimRewards);
router.get('/positions', stakingController.getAllPositions);
router.get('/positions/:userAddress', stakingController.getPosition);

export default router;
