import { createContext } from "react";

/** Available theme modes. */
type Theme = "light" | "dark";

/** Value provided by the ThemeContext. */
export interface ThemeContextValue {
	/** The current theme mode. */
	theme: Theme;
	/** Toggles between light and dark themes. */
	toggleTheme: () => void;
	/** Sets a specific theme. */
	setTheme: (theme: Theme) => void;
}

/** React context for theme state management. */
export const ThemeContext = createContext<ThemeContextValue | null>(null);
