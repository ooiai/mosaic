import express, { Request, Response } from 'express';
import { aiService } from '../services/ai.service';

export const generateCodeRouter = express.Router();

interface GenerateCodeRequest {
  prompt: string;
  framework?: string;
  style?: string;
}

generateCodeRouter.post('/', async (req: Request, res: Response) => {
  try {
    const { prompt, framework, style } = req.body as GenerateCodeRequest;

    if (!prompt || prompt.trim().length === 0) {
      return res.status(400).json({ 
        error: 'Prompt is required',
        message: 'Please provide a description of what you want to create'
      });
    }

    console.log(`Generating code for: "${prompt}"`);

    const generatedCode = await aiService.generateCode({
      prompt,
      framework: framework || 'react',
      style: style || 'modern',
    });

    res.json({
      success: true,
      code: generatedCode,
      framework: framework || 'react',
      prompt,
    });
  } catch (error) {
    console.error('Error generating code:', error);
    res.status(500).json({
      error: 'Failed to generate code',
      message: error instanceof Error ? error.message : 'Unknown error',
    });
  }
});

export default generateCodeRouter;
