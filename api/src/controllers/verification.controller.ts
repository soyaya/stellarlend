import { Request, Response } from 'express';
import { exec } from 'child_process';
import { promisify } from 'util';
import path from 'path';

const execAsync = promisify(exec);

/**
 * Verify contract against source code
 */
export const verifyContract = async (req: Request, res: Response): Promise<void> => {
  try {
    const { contractId, network = 'testnet' } = req.query;

    if (!contractId || typeof contractId !== 'string') {
      res.status(400).json({
        error: 'contractId parameter is required'
      });
      return;
    }

    // Determine source path based on contract ID
    // This is a simple mapping - in production, this might be stored in DB
    let sourcePath: string;
    if (contractId.startsWith('C') && contractId.length === 56) {
      // For now, assume it's the lending contract
      // In future, could query deployment manifest or database
      sourcePath = path.join(process.cwd(), '../../stellar-lend/contracts/hello-world');
    } else {
      res.status(400).json({
        error: 'Unable to determine source path for contract ID'
      });
      return;
    }

    const scriptPath = path.join(process.cwd(), '../../scripts/verify-contract.sh');
    const command = `${scriptPath} --contract-id ${contractId} --source ${sourcePath} --network ${network}`;

    const { stdout, stderr } = await execAsync(command);

    if (stderr && !stdout.includes('VERIFICATION SUCCESSFUL')) {
      res.status(400).json({
        verified: false,
        error: stderr
      });
      return;
    }

    res.json({
      verified: true,
      contractId,
      network,
      message: 'Contract verification successful'
    });

  } catch (error) {
    console.error('Verification error:', error);
    res.status(500).json({
      verified: false,
      error: 'Verification failed'
    });
  }
};