import { Router } from 'express';
import * as zkProofController from '../controllers/zkProof.controller';

const router: Router = Router();

router.post('/commit', zkProofController.commit);
router.post('/range-proof', zkProofController.rangeProof);
router.post('/verify-range', zkProofController.verifyRange);
router.post('/transfer-proof', zkProofController.transferProof);

export default router;
