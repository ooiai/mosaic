import React from 'react';

interface CodeEditorProps {
  code: string;
  onChange: (code: string) => void;
  isLoading: boolean;
}

export const CodeEditor: React.FC<CodeEditorProps> = ({ code, onChange, isLoading }) => {
  return (
    <div style={{
      background: 'white',
      borderRadius: '0.75rem',
      boxShadow: '0 4px 6px rgba(0, 0, 0, 0.1)',
      display: 'flex',
      flexDirection: 'column',
      height: '400px',
      overflow: 'hidden',
    }}>
      {/* Header */}
      <div style={{
        padding: '1rem 1.5rem',
        borderBottom: '1px solid #e5e7eb',
        background: '#f9fafb',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
      }}>
        <h2 style={{
          fontSize: '1.25rem',
          fontWeight: '600',
          color: '#1f2937',
        }}>
          Code Editor
        </h2>
        <button
          onClick={() => {
            navigator.clipboard.writeText(code);
            alert('Code copied to clipboard!');
          }}
          disabled={!code}
          style={{
            padding: '0.375rem 0.75rem',
            borderRadius: '0.375rem',
            border: '1px solid #d1d5db',
            background: 'white',
            color: '#374151',
            fontSize: '0.875rem',
            cursor: code ? 'pointer' : 'not-allowed',
            opacity: code ? 1 : 0.5,
          }}
        >
          Copy Code
        </button>
      </div>

      {/* Editor */}
      <textarea
        value={code}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Generated code will appear here..."
        disabled={isLoading}
        style={{
          flex: 1,
          padding: '1.5rem',
          border: 'none',
          outline: 'none',
          fontFamily: 'Monaco, Menlo, "Ubuntu Mono", monospace',
          fontSize: '0.875rem',
          lineHeight: '1.6',
          resize: 'none',
          background: '#1e293b',
          color: '#e2e8f0',
        }}
      />
    </div>
  );
};
