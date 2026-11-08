/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: "#0f1115",
        panel: "#171a21",
        line: "#262a34",
        accent: "#7c9cff",
      },
    },
  },
  plugins: [],
};
