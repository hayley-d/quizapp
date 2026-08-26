import { useEffect, useState } from "react"
import { Toaster as Sonner, type ToasterProps } from "sonner"
import { CircleCheckIcon, InfoIcon, TriangleAlertIcon, OctagonXIcon, Loader2Icon } from "lucide-react"

// The app's own theme source of truth is the `.dark` class on <html>, set by
// the inline script in index.html and kept live on OS-theme changes. Sonner
// has no way to read that class itself, so mirror it here instead of using
// next-themes (which nothing in the app actually provides a ThemeProvider
// for, and which resolves "system" independently of our own class).
function useDarkClass() {
  const [isDark, setIsDark] = useState(
    () => document.documentElement.classList.contains("dark"),
  )

  useEffect(() => {
    const root = document.documentElement
    const observer = new MutationObserver(() => {
      setIsDark(root.classList.contains("dark"))
    })
    observer.observe(root, { attributes: true, attributeFilter: ["class"] })
    return () => observer.disconnect()
  }, [])

  return isDark
}

const Toaster = ({ ...props }: ToasterProps) => {
  const theme: ToasterProps["theme"] = useDarkClass() ? "dark" : "light"

  return (
    <Sonner
      theme={theme}
      className="toaster group"
      icons={{
        success: (
          <CircleCheckIcon className="size-4" />
        ),
        info: (
          <InfoIcon className="size-4" />
        ),
        warning: (
          <TriangleAlertIcon className="size-4" />
        ),
        error: (
          <OctagonXIcon className="size-4" />
        ),
        loading: (
          <Loader2Icon className="size-4 animate-spin" />
        ),
      }}
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
          "--border-radius": "var(--radius)",
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          toast: "cn-toast",
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
