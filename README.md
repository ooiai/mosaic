# Mosaic 🎨

Mosaic is an AI-powered visual frontend development platform that revolutionizes how developers create user interfaces. Through conversational AI, developers can generate, modify, and iterate on frontend code with natural language, making UI development more intuitive and efficient.

## Features

✨ **AI-Powered Code Generation** - Describe your UI in plain English and let AI generate production-ready code

🔄 **Intelligent Code Modification** - Iterate on your designs by simply describing the changes you want

👁️ **Live Preview** - See your components render in real-time as you generate and modify them

🎯 **Multiple Frameworks** - Support for React, Vue, and plain HTML/CSS

💬 **Conversational Interface** - Natural language interaction makes frontend development accessible to everyone

⚡ **Modern Tech Stack** - Built with TypeScript, React, Node.js, and OpenAI

## Quick Start

### Prerequisites

- Node.js 18+ installed
- An OpenAI API key (optional - falls back to mock responses for testing)

### Installation

1. Clone the repository:
```bash
git clone https://github.com/ooiai/mosaic.git
cd mosaic
```

2. Install dependencies:
```bash
npm install
```

3. Configure environment variables:
```bash
cp .env.example .env
# Edit .env and add your OpenAI API key
```

4. Start the development server:
```bash
npm run dev
```

The application will be available at:
- Frontend: http://localhost:3000
- Backend API: http://localhost:3001

## Usage

### Generating Code

1. Select your preferred framework (React, Vue, or HTML)
2. In the chat interface, describe the component you want to create:
   - "Create a landing page hero section with a gradient background"
   - "Build a pricing table with three tiers"
   - "Make a login form with email and password fields"
3. The AI will generate the code and display it in the editor
4. View the live preview on the right side

### Modifying Code

1. After generating code, switch to "Modify" mode in the chat interface
2. Describe the changes you want:
   - "Make the button blue instead of green"
   - "Add a subtitle below the main heading"
   - "Change the layout to use flexbox"
3. The AI will update the code based on your instructions

### Editing Code Manually

You can also manually edit the generated code in the code editor. Changes will be reflected in the live preview.

## Architecture

```
mosaic/
├── src/
│   ├── server/              # Backend API
│   │   ├── index.ts         # Express server
│   │   ├── routes/          # API endpoints
│   │   └── services/        # Business logic & AI integration
│   └── client/              # Frontend React app
│       ├── App.tsx          # Main application component
│       ├── components/      # React components
│       ├── services/        # API client
│       └── styles/          # Global styles
├── package.json
└── README.md
```

## API Endpoints

### POST /api/generate
Generate code from a natural language prompt.

**Request:**
```json
{
  "prompt": "Create a hero section with a call-to-action button",
  "framework": "react",
  "style": "modern"
}
```

**Response:**
```json
{
  "success": true,
  "code": "import React from 'react'...",
  "framework": "react",
  "prompt": "Create a hero section..."
}
```

### POST /api/modify
Modify existing code based on instructions.

**Request:**
```json
{
  "code": "existing code...",
  "instruction": "Change the button color to blue"
}
```

**Response:**
```json
{
  "success": true,
  "code": "modified code...",
  "instruction": "Change the button color to blue"
}
```

### GET /api/health
Check API health status.

## Scripts

- `npm run dev` - Start development server (both frontend and backend)
- `npm run dev:server` - Start backend server only
- `npm run dev:client` - Start frontend only
- `npm run build` - Build for production
- `npm run start` - Start production server
- `npm run lint` - Run ESLint
- `npm test` - Run tests

## Configuration

### Environment Variables

Create a `.env` file in the root directory:

```env
# OpenAI API Configuration
OPENAI_API_KEY=your_openai_api_key_here

# Server Configuration
PORT=3001
NODE_ENV=development

# Client Configuration
VITE_API_URL=http://localhost:3001
```

## Technology Stack

### Frontend
- **React 18** - UI library
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **Axios** - HTTP client

### Backend
- **Node.js** - Runtime environment
- **Express** - Web framework
- **TypeScript** - Type safety
- **OpenAI API** - AI code generation

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with ❤️ by the ooiailab team
- Powered by OpenAI's GPT models
- Inspired by the vision of making frontend development more accessible

## Support

For issues, questions, or suggestions, please open an issue on GitHub.
