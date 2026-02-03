import axios from 'axios';

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3001';

export interface GenerateCodeRequest {
  prompt: string;
  framework?: string;
  style?: string;
}

export interface GenerateCodeResponse {
  success: boolean;
  code: string;
  framework: string;
  prompt: string;
}

export interface ModifyCodeRequest {
  code: string;
  instruction: string;
}

export interface ModifyCodeResponse {
  success: boolean;
  code: string;
  instruction: string;
}

export const apiService = {
  async generateCode(request: GenerateCodeRequest): Promise<GenerateCodeResponse> {
    const response = await axios.post<GenerateCodeResponse>(
      `${API_URL}/api/generate`,
      request
    );
    return response.data;
  },

  async modifyCode(request: ModifyCodeRequest): Promise<ModifyCodeResponse> {
    const response = await axios.post<ModifyCodeResponse>(
      `${API_URL}/api/modify`,
      request
    );
    return response.data;
  },

  async checkHealth(): Promise<{ status: string; message: string }> {
    const response = await axios.get(`${API_URL}/api/health`);
    return response.data;
  },
};
