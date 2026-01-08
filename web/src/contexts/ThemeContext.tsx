import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react";

type Theme = "light" | "dark";

interface ThemeContextValue {
	theme: Theme;
	toggleTheme: () => void;
	setTheme: (theme: Theme) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

const THEME_STORAGE_KEY = "sass-dep-theme";

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
		const handleChange = (e: MediaQueryListEvent) => {
			// Only auto-switch if user hasn't set a preference
			const stored = localStorage.getItem(THEME_STORAGE_KEY);
			if (!stored) {
				setThemeState(e.matches ? "dark" : "light");
			}
		};

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

export function useTheme() {
	const context = useContext(ThemeContext);
	if (!context) {
		throw new Error("useTheme must be used within a ThemeProvider");
	}
	return context;
}
