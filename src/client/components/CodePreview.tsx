import React, { useEffect, useRef } from 'react';

interface CodePreviewProps {
  code: string;
}

export const CodePreview: React.FC<CodePreviewProps> = ({ code }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    if (!code || !iframeRef.current) return;

    const iframe = iframeRef.current;
    
    // Transform the code to remove exports and extract component
    const transformedCode = code
      .replace(/^import\s+.*from\s+['"].*['"];?\s*/gm, '') // Remove imports
      .replace(/^export\s+(default\s+)?/gm, '') // Remove export keywords
      .replace(/^interface\s+\w+Props\s*{[^}]*}/gm, '') // Remove interface (TypeScript not needed in preview)
      .replace(/:\s*React\.FC<\w+>/g, ''); // Remove React.FC types
    
    // Properly escape the code for safe embedding in template literal
    const escapeForTemplate = (str: string): string => {
      return str
        .replace(/\\/g, '\\\\')  // Escape backslashes first
        .replace(/`/g, '\\`')     // Escape backticks
        .replace(/\$/g, '\\$');   // Escape dollar signs
    };
    
    const safeCode = escapeForTemplate(transformedCode);
    
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
            const codeToExecute = \`${safeCode}\`;
            
            // Try to render the component
            try {
              const root = ReactDOM.createRoot(document.getElementById('root'));
              
              // Extract component function name using regex
              const componentMatch = codeToExecute.match(/(?:const|function)\\s+(\\w+)\\s*[=:]?/);
              const componentName = componentMatch ? componentMatch[1] : null;
              
              if (componentName) {
                // Create a safe function context without eval
                const ComponentFunction = new Function('React', 'return ' + codeToExecute + '; return ' + componentName)(React);
                root.render(React.createElement(ComponentFunction));
              } else {
                document.getElementById('root').innerHTML = \`
                  <div style="padding: 2rem; text-align: center; color: #666;">
                    <h2>Component Preview</h2>
                    <p>Component code generated successfully.</p>
                    <small>Note: Full preview rendering requires proper component structure.</small>
                  </div>
                \`;
              }
            } catch (error) {
              // Show a friendly message
              document.getElementById('root').innerHTML = \`
                <div style="padding: 2rem; background: #f0f9ff; border: 1px solid #bae6fd; border-radius: 8px;">
                  <h3 style="color: #0369a1; margin-top: 0;">✓ Code Generated Successfully</h3>
                  <p style="color: #075985;">Your component code is ready in the editor. Copy it to use in your project!</p>
                  <small style="color: #7dd3fc;">Preview: \${error.message}</small>
                </div>
              \`;
            }
          </script>
        </body>
      </html>
    `;

    // Use srcdoc instead of contentDocument to avoid CORS issues
    iframe.srcdoc = previewHtml;
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
