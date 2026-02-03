import OpenAI from 'openai';

interface GenerateCodeParams {
  prompt: string;
  framework?: string;
  style?: string;
}

interface ModifyCodeParams {
  code: string;
  instruction: string;
}

export class AIService {
  private openai: OpenAI | null = null;
  private isConfigured: boolean = false;

  constructor() {
    const apiKey = process.env.OPENAI_API_KEY;
    if (apiKey && apiKey !== 'your_openai_api_key_here') {
      this.openai = new OpenAI({ apiKey });
      this.isConfigured = true;
    } else {
      console.warn('⚠️  OpenAI API key not configured. Using mock responses.');
    }
  }

  async generateCode({ prompt, framework = 'react', style = 'modern' }: GenerateCodeParams): Promise<string> {
    if (!this.isConfigured || !this.openai) {
      return this.getMockGeneratedCode(prompt, framework);
    }

    try {
      const systemPrompt = `You are an expert frontend developer. Generate clean, modern, production-ready ${framework} code based on user requirements. 
- Use TypeScript when applicable
- Follow best practices and modern patterns
- Include proper styling with ${style} design principles
- Make code reusable and well-structured
- Only return the code, no explanations`;

      const response = await this.openai.chat.completions.create({
        model: 'gpt-4',
        messages: [
          { role: 'system', content: systemPrompt },
          { role: 'user', content: prompt }
        ],
        temperature: 0.7,
        max_tokens: 2000,
      });

      return response.choices[0]?.message?.content || this.getMockGeneratedCode(prompt, framework);
    } catch (error) {
      console.error('Error calling OpenAI API:', error);
      return this.getMockGeneratedCode(prompt, framework);
    }
  }

  async modifyCode({ code, instruction }: ModifyCodeParams): Promise<string> {
    if (!this.isConfigured || !this.openai) {
      return this.getMockModifiedCode(code, instruction);
    }

    try {
      const systemPrompt = `You are an expert frontend developer. Modify the provided code based on the user's instruction.
- Maintain the existing code structure where possible
- Apply the requested changes precisely
- Keep code clean and follow best practices
- Only return the modified code, no explanations`;

      const response = await this.openai.chat.completions.create({
        model: 'gpt-4',
        messages: [
          { role: 'system', content: systemPrompt },
          { role: 'user', content: `Current code:\n\`\`\`\n${code}\n\`\`\`\n\nInstruction: ${instruction}` }
        ],
        temperature: 0.5,
        max_tokens: 2000,
      });

      return response.choices[0]?.message?.content || this.getMockModifiedCode(code, instruction);
    } catch (error) {
      console.error('Error calling OpenAI API:', error);
      return this.getMockModifiedCode(code, instruction);
    }
  }

  private getMockGeneratedCode(prompt: string, framework: string): string {
    const componentName = this.extractComponentName(prompt);
    
    if (framework === 'react') {
      return `import React from 'react';

interface ${componentName}Props {
  title?: string;
}

export const ${componentName}: React.FC<${componentName}Props> = ({ title = 'Welcome' }) => {
  return (
    <div style={{
      padding: '2rem',
      maxWidth: '800px',
      margin: '0 auto',
      fontFamily: 'system-ui, -apple-system, sans-serif'
    }}>
      <h1 style={{
        fontSize: '2.5rem',
        color: '#1a202c',
        marginBottom: '1rem'
      }}>
        {title}
      </h1>
      <p style={{
        fontSize: '1.125rem',
        color: '#4a5568',
        lineHeight: '1.75'
      }}>
        This component was generated based on: "{prompt}"
      </p>
      <button style={{
        marginTop: '1.5rem',
        padding: '0.75rem 1.5rem',
        backgroundColor: '#3182ce',
        color: 'white',
        border: 'none',
        borderRadius: '0.375rem',
        fontSize: '1rem',
        cursor: 'pointer'
      }}>
        Get Started
      </button>
    </div>
  );
};

export default ${componentName};`;
    }

    return `<!-- Generated HTML for: ${prompt} -->
<div class="container">
  <h1>${componentName}</h1>
  <p>Generated based on your request: ${prompt}</p>
</div>`;
  }

  private getMockModifiedCode(code: string, instruction: string): string {
    // Simple mock modification - add a comment
    return `// Modified: ${instruction}\n${code}`;
  }

  private extractComponentName(prompt: string): string {
    // Extract meaningful words and create a component name
    const words = prompt
      .split(' ')
      .filter(w => w.length > 2 && !['the', 'and', 'for', 'with'].includes(w.toLowerCase()))
      .map(w => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase());
    
    // Take first 2-3 words or fallback to generic name
    const name = words.slice(0, Math.min(3, words.length)).join('');
    return name && name.length >= 4 ? name : 'GeneratedComponent';
  }
}

export const aiService = new AIService();
