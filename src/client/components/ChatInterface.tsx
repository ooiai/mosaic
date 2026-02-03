import React, { useState } from 'react';

interface ChatInterfaceProps {
  onGenerate: (prompt: string) => void;
  onModify: (instruction: string) => void;
  isLoading: boolean;
}

export const ChatInterface: React.FC<ChatInterfaceProps> = ({ 
  onGenerate, 
  onModify, 
  isLoading 
}) => {
  const [input, setInput] = useState('');
  const [mode, setMode] = useState<'generate' | 'modify'>('generate');
  const [messages, setMessages] = useState<Array<{ type: 'user' | 'system'; text: string }>>([
    { 
      type: 'system', 
      text: 'Welcome to Mosaic! Describe the UI component you want to create, and I\'ll generate the code for you.' 
    }
  ]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || isLoading) return;

    setMessages(prev => [...prev, { type: 'user', text: input }]);

    if (mode === 'generate') {
      onGenerate(input);
      setMessages(prev => [...prev, { 
        type: 'system', 
        text: 'Generating your component...' 
      }]);
    } else {
      onModify(input);
      setMessages(prev => [...prev, { 
        type: 'system', 
        text: 'Modifying your code...' 
      }]);
    }

    setInput('');
  };

  return (
    <div style={{
      background: 'white',
      borderRadius: '0.75rem',
      boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      overflow: 'hidden',
    }}>
      {/* Header */}
      <div style={{
        padding: '1rem 1.5rem',
        borderBottom: '1px solid #e5e7eb',
        background: '#f9fafb',
      }}>
        <h2 style={{
          fontSize: '1.25rem',
          fontWeight: '600',
          color: '#1f2937',
          marginBottom: '0.5rem',
        }}>
          Conversational Interface
        </h2>
        <div style={{ display: 'flex', gap: '0.5rem' }}>
          <button
            onClick={() => setMode('generate')}
            style={{
              padding: '0.375rem 0.75rem',
              borderRadius: '0.375rem',
              border: 'none',
              background: mode === 'generate' ? '#3b82f6' : '#e5e7eb',
              color: mode === 'generate' ? 'white' : '#6b7280',
              fontSize: '0.875rem',
              cursor: 'pointer',
              fontWeight: '500',
            }}
          >
            Generate
          </button>
          <button
            onClick={() => setMode('modify')}
            style={{
              padding: '0.375rem 0.75rem',
              borderRadius: '0.375rem',
              border: 'none',
              background: mode === 'modify' ? '#3b82f6' : '#e5e7eb',
              color: mode === 'modify' ? 'white' : '#6b7280',
              fontSize: '0.875rem',
              cursor: 'pointer',
              fontWeight: '500',
            }}
          >
            Modify
          </button>
        </div>
      </div>

      {/* Messages */}
      <div style={{
        flex: 1,
        overflowY: 'auto',
        padding: '1.5rem',
        display: 'flex',
        flexDirection: 'column',
        gap: '1rem',
      }}>
        {messages.map((msg, idx) => (
          <div
            key={idx}
            style={{
              alignSelf: msg.type === 'user' ? 'flex-end' : 'flex-start',
              maxWidth: '80%',
            }}
          >
            <div style={{
              padding: '0.75rem 1rem',
              borderRadius: '0.75rem',
              background: msg.type === 'user' ? '#3b82f6' : '#f3f4f6',
              color: msg.type === 'user' ? 'white' : '#1f2937',
              fontSize: '0.9rem',
              lineHeight: '1.5',
            }}>
              {msg.text}
            </div>
          </div>
        ))}
      </div>

      {/* Input */}
      <form onSubmit={handleSubmit} style={{
        padding: '1rem 1.5rem',
        borderTop: '1px solid #e5e7eb',
        background: '#f9fafb',
      }}>
        <div style={{
          display: 'flex',
          gap: '0.75rem',
        }}>
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={
              mode === 'generate' 
                ? "Describe the UI you want to create..." 
                : "Describe how to modify the code..."
            }
            disabled={isLoading}
            style={{
              flex: 1,
              padding: '0.75rem 1rem',
              borderRadius: '0.5rem',
              border: '1px solid #d1d5db',
              fontSize: '0.9rem',
              outline: 'none',
            }}
          />
          <button
            type="submit"
            disabled={isLoading || !input.trim()}
            style={{
              padding: '0.75rem 1.5rem',
              borderRadius: '0.5rem',
              border: 'none',
              background: isLoading || !input.trim() ? '#9ca3af' : '#3b82f6',
              color: 'white',
              fontSize: '0.9rem',
              fontWeight: '500',
              cursor: isLoading || !input.trim() ? 'not-allowed' : 'pointer',
            }}
          >
            {isLoading ? 'Processing...' : 'Send'}
          </button>
        </div>
      </form>
    </div>
  );
};
