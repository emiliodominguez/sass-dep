import { type ReactNode, useCallback, useEffect, useState } from "react";

import { ThemeContext } from "./themeContext";

type Theme = "light" | "dark";

/** localStorage key for persisting theme preference. */
const THEME_STORAGE_KEY = "sass-dep-theme";

/**
 * Gets the initial theme from localStorage or system preference.
 * @returns The initial theme to use
 */
function getInitialTheme(): Theme {
	// Check localStorage first
	if (typeof window !== "undefined") {
		const stored = localStorage.getItem(THEME_STORAGE_KEY);
		if (stored === "light" || stored === "dark") {
			return stored;
		}
		// Check system preference
		if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
			return "dark";
		}
	}
	return "light";
}

interface ThemeProviderProps {
	children: ReactNode;
}

/**
 * Provider component for theme context.
 * Manages theme state, persistence, and system preference detection.
 * @param props - Component props containing children
 * @returns Provider wrapping children with theme context
 */
export function ThemeProvider({ children }: ThemeProviderProps) {
	const [theme, setThemeState] = useState<Theme>(getInitialTheme);

	useEffect(() => {
		// Apply theme to document
		document.documentElement.setAttribute("data-theme", theme);
		localStorage.setItem(THEME_STORAGE_KEY, theme);
	}, [theme]);

	// Listen for system theme changes
	useEffect(() => {
		const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

		/**
		 * Handles changes in system theme preference.
		 * @param e - Media query event
		 */
		function handleChange(e: MediaQueryListEvent): void {
			// Only auto-switch if user hasn't set a preference
			const stored = localStorage.getItem(THEME_STORAGE_KEY);

			if (!stored) {
				setThemeState(e.matches ? "dark" : "light");
			}
		}

		mediaQuery.addEventListener("change", handleChange);

		return () => mediaQuery.removeEventListener("change", handleChange);
	}, []);

	const toggleTheme = useCallback(() => {
		setThemeState((prev) => (prev === "light" ? "dark" : "light"));
	}, []);

	const setTheme = useCallback((newTheme: Theme) => {
		setThemeState(newTheme);
	}, []);

	return <ThemeContext.Provider value={{ theme, toggleTheme, setTheme }}>{children}</ThemeContext.Provider>;
}
