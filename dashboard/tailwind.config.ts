import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./app/**/*.{ts,tsx}",
    "./components/**/*.{ts,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        navy: {
          950: "#101518",  // body bg (--yb-midnight)
          900: "#1d2228",  // card bg (--yb-inkwell)
          800: "#232a31",  // elevated bg (--yb-batcave)
          700: "#2c363f",  // hover (--yb-ramones)
          600: "#3a434c",  // borders
          500: "#4e5964",  // muted
        },
        accent: {
          purple: "#9d61ff",       // --yb-grape-jelly dark
          "purple-light": "#b88aff",
          "purple-dark": "#7c3fe6",
          blue: "#12a9ff",         // --yb-sky
        },
        gain: "#21d87d",           // --yb-sa-stock-up
        loss: "#fc7a6e",           // --yb-sa-stock-down
        "text-primary": "#f0f3f5", // --yb-gray-hair
        "text-secondary": "#b0b9c1", // --yb-bob
      },
      fontFamily: {
        sans: [
          '"Helvetica Neue"',
          'Helvetica',
          'Arial',
          'sans-serif',
        ],
      },
    },
  },
  plugins: [],
};

export default config;
