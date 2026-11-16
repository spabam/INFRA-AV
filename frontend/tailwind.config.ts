import type { Config } from "tailwindcss";
import tailwindcssAnimate from "tailwindcss-animate";

// Semantic colors retain Tailwind opacity modifiers without a second palette.
const color = (token: string) => `color-mix(in srgb, var(--u-${token}) calc(<alpha-value> * 100%), transparent)`;

export default {
  darkMode: ["class"],
  content: ["./pages/**/*.{ts,tsx}", "./components/**/*.{ts,tsx}", "./app/**/*.{ts,tsx}", "./src/**/*.{ts,tsx}"],
  prefix: "",
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      colors: {
        border: color("border"),
        input: color("input"),
        ring: color("ring"),
        background: color("canvas"),
        foreground: color("text"),
        primary: {
          DEFAULT: color("primary"),
          foreground: color("primary-fg"),
        },
        secondary: {
          DEFAULT: color("surface-2"),
          foreground: color("text"),
        },
        destructive: {
          DEFAULT: color("danger"),
          foreground: color("danger-fg"),
        },
        success: {
          DEFAULT: color("ok"),
          foreground: color("ok-fg"),
        },
        warning: {
          DEFAULT: color("warn"),
          foreground: color("warn-fg"),
        },
        muted: {
          DEFAULT: color("surface-2"),
          foreground: color("muted"),
        },
        accent: {
          DEFAULT: color("accent-soft"),
          foreground: color("accent"),
        },
        popover: {
          DEFAULT: color("surface"),
          foreground: color("text"),
        },
        card: {
          DEFAULT: color("surface"),
          foreground: color("text"),
        },
        sidebar: {
          DEFAULT: color("surface"),
          foreground: color("text"),
          primary: color("primary"),
          "primary-foreground": color("primary-fg"),
          accent: color("accent-soft"),
          "accent-foreground": color("accent"),
          border: color("border"),
          ring: color("ring"),
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      fontFamily: {
        sans: ['var(--u-font)'],
        mono: ['var(--u-mono)'],
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [tailwindcssAnimate],
} satisfies Config;
