import { Router } from 'express';
import * as merkleController from '../controllers/merkle.controller';

const router: Router = Router();

router.post('/accounts', merkleController.upsertAccount);
router.get('/accounts', merkleController.listAccounts);
router.get('/accounts/:userAddress', merkleController.getAccount);
router.get('/proof/:userAddress', merkleController.getProof);
router.post('/verify', merkleController.verifyProof);
router.get('/tree', merkleController.getTreeInfo);

export default router;
