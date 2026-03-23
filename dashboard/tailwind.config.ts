import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./app/**/*.{ts,tsx}",
    "./components/**/*.{ts,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        surface: {
          950: "#0c0d10",  // body bg
          900: "#14151a",  // card bg
          800: "#1a1b22",  // elevated bg / inputs
          700: "#22232b",  // hover
          600: "#2e2f38",  // borders
          500: "#3e3f4a",  // muted / scrollbar
        },
        accent: {
          DEFAULT: "#06b6d4",  // cyan-500
          light: "#22d3ee",    // cyan-400
          dark: "#0891b2",     // cyan-600
        },
        gain: "#34d399",       // emerald-400
        loss: "#f87171",       // red-400
        "text-primary": "#e4e4e7",   // zinc-200
        "text-secondary": "#8b8d98", // muted slate
      },
      fontFamily: {
        sans: ["var(--font-sans)", "system-ui", "sans-serif"],
        mono: ["var(--font-mono)", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};

export default config;
