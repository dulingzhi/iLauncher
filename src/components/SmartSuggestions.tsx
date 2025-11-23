import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Suggestion {
  result_id: string;
  title: string;
  subtitle: string;
  score: number;
  reason: string;
}

interface SmartSuggestionsProps {
  query: string;
  onSelect: (suggestion: Suggestion) => void;
}

export function SmartSuggestions({ query, onSelect }: SmartSuggestionsProps) {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadSuggestions();
  }, [query]);

  const loadSuggestions = async () => {
    if (query.trim()) return; // 仅在空查询时显示推荐
    
    setLoading(true);
    try {
      const results = await invoke<Suggestion[]>("get_smart_suggestions", {
        query: "",
        limit: 8,
      });
      setSuggestions(results);
    } catch (error) {
      console.error("Failed to load suggestions:", error);
    } finally {
      setLoading(false);
    }
  };

  if (query.trim() || suggestions.length === 0) {
    return null;
  }

  return (
    <div style={{
      padding: "16px",
      maxWidth: "800px",
      margin: "0 auto",
    }}>
      <h3 style={{
        fontSize: "14px",
        fontWeight: 600,
        color: "var(--color-text-secondary)",
        marginBottom: "12px",
        opacity: 0.8,
      }}>
        💡 智能推荐
      </h3>
      
      <div style={{
        display: "grid",
        gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))",
        gap: "8px",
      }}>
        {suggestions.map((suggestion) => (
          <div
            key={suggestion.result_id}
            onClick={() => onSelect(suggestion)}
            style={{
              padding: "12px",
              background: "var(--color-surface)",
              borderRadius: "8px",
              border: "1px solid var(--color-border)",
              cursor: "pointer",
              transition: "all 0.2s ease",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = "var(--color-hover)";
              e.currentTarget.style.borderColor = "var(--color-primary)";
              e.currentTarget.style.transform = "translateY(-2px)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "var(--color-surface)";
              e.currentTarget.style.borderColor = "var(--color-border)";
              e.currentTarget.style.transform = "translateY(0)";
            }}
          >
            <div style={{
              fontSize: "13px",
              fontWeight: 500,
              color: "var(--color-text-primary)",
              marginBottom: "4px",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}>
              {suggestion.title}
            </div>
            
            <div style={{
              fontSize: "11px",
              color: "var(--color-text-muted)",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}>
              {suggestion.subtitle}
            </div>
            
            <div style={{
              marginTop: "6px",
              fontSize: "10px",
              color: "var(--color-primary)",
              opacity: 0.7,
            }}>
              {suggestion.reason}
            </div>
          </div>
        ))}
      </div>
      
      {loading && (
        <div style={{
          textAlign: "center",
          padding: "20px",
          color: "var(--color-text-muted)",
          fontSize: "12px",
        }}>
          加载推荐中...
        </div>
      )}
    </div>
  );
}
