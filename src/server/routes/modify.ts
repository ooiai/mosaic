import express, { Request, Response } from 'express';
import { aiService } from '../services/ai.service';

export const modifyCodeRouter = express.Router();

interface ModifyCodeRequest {
  code: string;
  instruction: string;
}

modifyCodeRouter.post('/', async (req: Request, res: Response) => {
  try {
    const { code, instruction } = req.body as ModifyCodeRequest;

    if (!code || code.trim().length === 0) {
      return res.status(400).json({ 
        error: 'Code is required',
        message: 'Please provide the code you want to modify'
      });
    }

    if (!instruction || instruction.trim().length === 0) {
      return res.status(400).json({ 
        error: 'Instruction is required',
        message: 'Please provide instructions on how to modify the code'
      });
    }

    console.log(`Modifying code with instruction: "${instruction}"`);

    const modifiedCode = await aiService.modifyCode({
      code,
      instruction,
    });

    res.json({
      success: true,
      code: modifiedCode,
      instruction,
    });
  } catch (error) {
    console.error('Error modifying code:', error);
    res.status(500).json({
      error: 'Failed to modify code',
      message: error instanceof Error ? error.message : 'Unknown error',
    });
  }
});

export default modifyCodeRouter;
