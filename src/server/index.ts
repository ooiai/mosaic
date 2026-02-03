import express, { Request, Response, NextFunction } from 'express';
import cors from 'cors';
import dotenv from 'dotenv';
import { generateCodeRouter } from './routes/generate';
import { modifyCodeRouter } from './routes/modify';

dotenv.config();

const app = express();
const PORT = process.env.PORT || 3001;

// Middleware
app.use(cors());
app.use(express.json({ limit: '10mb' }));

// Health check endpoint
app.get('/api/health', (req: Request, res: Response) => {
  res.json({ status: 'ok', message: 'Mosaic API is running' });
});

// API Routes
app.use('/api/generate', generateCodeRouter);
app.use('/api/modify', modifyCodeRouter);

// Error handling middleware
app.use((err: Error, req: Request, res: Response, next: NextFunction) => {
  console.error('Error:', err);
  res.status(500).json({ 
    error: 'Internal server error', 
    message: err.message 
  });
});

app.listen(PORT, () => {
  console.log(`🚀 Mosaic server running on http://localhost:${PORT}`);
  console.log(`📝 API available at http://localhost:${PORT}/api`);
});

export default app;
