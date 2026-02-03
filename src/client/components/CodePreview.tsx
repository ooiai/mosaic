import React, { useEffect, useRef } from 'react';

interface CodePreviewProps {
  code: string;
}

export const CodePreview: React.FC<CodePreviewProps> = ({ code }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    if (!code || !iframeRef.current) return;

    const iframe = iframeRef.current;
    const iframeDoc = iframe.contentDocument || iframe.contentWindow?.document;

    if (!iframeDoc) return;

    // Create preview HTML with the generated code
    const previewHtml = `
      <!DOCTYPE html>
      <html>
        <head>
          <meta charset="UTF-8">
          <meta name="viewport" content="width=device-width, initial-scale=1.0">
          <script crossorigin src="https://unpkg.com/react@18/umd/react.development.js"></script>
          <script crossorigin src="https://unpkg.com/react-dom@18/umd/react-dom.development.js"></script>
          <script src="https://unpkg.com/@babel/standalone/babel.min.js"></script>
          <style>
            body {
              margin: 0;
              padding: 1rem;
              font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
              background: #f9fafb;
            }
          </style>
        </head>
        <body>
          <div id="root"></div>
          <script type="text/babel">
            ${code}
            
            // Try to render if it's a React component
            try {
              const root = ReactDOM.createRoot(document.getElementById('root'));
              
              // Find the default export or the first exported component
              const ComponentToRender = typeof module !== 'undefined' && module.exports 
                ? module.exports.default || module.exports 
                : window.GeneratedComponent || (() => {
                    return React.createElement('div', null, 'Component loaded');
                  });
              
              root.render(React.createElement(ComponentToRender));
            } catch (error) {
              document.getElementById('root').innerHTML = '<div style="color: red; padding: 1rem;">Preview Error: ' + error.message + '</div>';
            }
          </script>
        </body>
      </html>
    `;

    iframeDoc.open();
    iframeDoc.write(previewHtml);
    iframeDoc.close();
  }, [code]);

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
      }}>
        <h2 style={{
          fontSize: '1.25rem',
          fontWeight: '600',
          color: '#1f2937',
        }}>
          Live Preview
        </h2>
      </div>

      {/* Preview */}
      {code ? (
        <iframe
          ref={iframeRef}
          title="Code Preview"
          style={{
            flex: 1,
            border: 'none',
            width: '100%',
            background: 'white',
          }}
          sandbox="allow-scripts"
        />
      ) : (
        <div style={{
          flex: 1,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          color: '#9ca3af',
          fontSize: '0.9rem',
        }}>
          Preview will appear here after code generation
        </div>
      )}
    </div>
  );
};
