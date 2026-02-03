import React, { useState } from 'react';
import { CodeEditor } from './components/CodeEditor';
import { ChatInterface } from './components/ChatInterface';
import { CodePreview } from './components/CodePreview';
import { apiService } from './services/api.service';

const App: React.FC = () => {
  const [generatedCode, setGeneratedCode] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [framework, setFramework] = useState<string>('react');

  const handleGenerateCode = async (prompt: string) => {
    setIsLoading(true);
    try {
      const response = await apiService.generateCode({ 
        prompt, 
        framework,
        style: 'modern' 
      });
      setGeneratedCode(response.code);
    } catch (error) {
      console.error('Error generating code:', error);
      alert('Failed to generate code. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleModifyCode = async (instruction: string) => {
    if (!generatedCode) {
      alert('Please generate code first before modifying.');
      return;
    }
    
    setIsLoading(true);
    try {
      const response = await apiService.modifyCode({ 
        code: generatedCode, 
        instruction 
      });
      setGeneratedCode(response.code);
    } catch (error) {
      console.error('Error modifying code:', error);
      alert('Failed to modify code. Please try again.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleCodeChange = (newCode: string) => {
    setGeneratedCode(newCode);
  };

  return (
    <div style={{
      minHeight: '100vh',
      display: 'flex',
      flexDirection: 'column',
      background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
    }}>
      {/* Header */}
      <header style={{
        padding: '1.5rem 2rem',
        background: 'rgba(255, 255, 255, 0.1)',
        backdropFilter: 'blur(10px)',
        borderBottom: '1px solid rgba(255, 255, 255, 0.2)',
      }}>
        <div style={{
          maxWidth: '1400px',
          margin: '0 auto',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}>
          <h1 style={{
            color: 'white',
            fontSize: '2rem',
            fontWeight: 'bold',
            display: 'flex',
            alignItems: 'center',
            gap: '0.75rem',
          }}>
            <span style={{ fontSize: '2.5rem' }}>🎨</span>
            Mosaic
          </h1>
          <div style={{
            display: 'flex',
            gap: '1rem',
            alignItems: 'center',
          }}>
            <label style={{ color: 'white', fontSize: '0.9rem' }}>
              Framework:
            </label>
            <select
              value={framework}
              onChange={(e) => setFramework(e.target.value)}
              style={{
                padding: '0.5rem 1rem',
                borderRadius: '0.375rem',
                border: '1px solid rgba(255, 255, 255, 0.3)',
                background: 'rgba(255, 255, 255, 0.2)',
                color: 'white',
                fontSize: '0.9rem',
                cursor: 'pointer',
              }}
            >
              <option value="react">React</option>
              <option value="vue">Vue</option>
              <option value="html">HTML</option>
            </select>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <div style={{
        flex: 1,
        display: 'grid',
        gridTemplateColumns: '1fr 1fr',
        gap: '1.5rem',
        padding: '1.5rem',
        maxWidth: '1400px',
        width: '100%',
        margin: '0 auto',
      }}>
        {/* Left Panel */}
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1.5rem',
          minHeight: 0,
        }}>
          <ChatInterface 
            onGenerate={handleGenerateCode}
            onModify={handleModifyCode}
            isLoading={isLoading}
          />
        </div>

        {/* Right Panel */}
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1.5rem',
          minHeight: 0,
        }}>
          <CodeEditor 
            code={generatedCode}
            onChange={handleCodeChange}
            isLoading={isLoading}
          />
          <CodePreview code={generatedCode} />
        </div>
      </div>

      {/* Footer */}
      <footer style={{
        padding: '1rem 2rem',
        textAlign: 'center',
        color: 'white',
        fontSize: '0.875rem',
        opacity: 0.8,
      }}>
        Mosaic - AI-Powered Visual Frontend Development Platform
      </footer>
    </div>
  );
};

export default App;
