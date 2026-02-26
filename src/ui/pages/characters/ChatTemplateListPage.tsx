import { useCallback, useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { Plus, MessageSquare, Trash2, MoreVertical, Loader2 } from "lucide-react";
import { listCharacters, saveCharacter } from "../../../core/storage/repo";
import type { Character } from "../../../core/storage/schemas";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";

export default function ChatTemplateListPage() {
  const { characterId } = useParams<{ characterId: string }>();
  const navigate = useNavigate();
  const [character, setCharacter] = useState<Character | null>(null);
  const [loading, setLoading] = useState(true);
  const [menuTemplateId, setMenuTemplateId] = useState<string | null>(null);

  const loadCharacter = useCallback(async () => {
    if (!characterId) return;
    try {
      const chars = await listCharacters();
      const c = chars.find((ch) => ch.id === characterId);
      if (c) setCharacter(c);
    } finally {
      setLoading(false);
    }
  }, [characterId]);

  useEffect(() => {
    loadCharacter();
  }, [loadCharacter]);

  const templates = character?.chatTemplates ?? [];

  const handleDelete = useCallback(
    async (templateId: string) => {
      if (!character) return;
      const updated = {
        ...character,
        chatTemplates: templates.filter((t) => t.id !== templateId),
      };
      await saveCharacter(updated);
      setCharacter(updated);
      setMenuTemplateId(null);
    },
    [character, templates],
  );

  const menuTemplate = templates.find((t) => t.id === menuTemplateId);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-fg/40" />
      </div>
    );
  }

  if (!character) {
    return (
      <div className="flex h-full items-center justify-center text-fg/50">Character not found</div>
    );
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className="space-y-4"
    >
      {templates.length > 0 && (
        <div className="flex items-center justify-between">
          <p className="text-xs text-fg/50">
            {templates.length} template{templates.length !== 1 ? "s" : ""} for {character.name}
          </p>
          <motion.button
            whileTap={{ scale: 0.95 }}
            onClick={() => navigate(`/settings/characters/${characterId}/templates/new`)}
            className="flex items-center gap-1.5 rounded-lg border border-accent/50 bg-accent/20 px-3 py-1.5 text-xs font-medium text-accent transition active:bg-accent/30"
          >
            <Plus className="h-3.5 w-3.5" />
            New Template
          </motion.button>
        </div>
      )}

      {templates.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="mb-3 rounded-2xl border border-fg/10 bg-fg/5 p-4">
            <MessageSquare className="h-8 w-8 text-fg/30" />
          </div>
          <p className="text-sm font-medium text-fg/60">No templates yet</p>
          <p className="mt-1 max-w-xs text-xs text-fg/40">
            Chat templates let you start conversations with pre-written messages from both you and{" "}
            {character.name}.
          </p>
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={() => navigate(`/settings/characters/${characterId}/templates/new`)}
            className="mt-4 flex items-center gap-2 rounded-xl border border-accent/50 bg-accent/20 px-4 py-2 text-sm font-medium text-accent transition active:bg-accent/30"
          >
            <Plus className="h-4 w-4" />
            Create Template
          </motion.button>
        </div>
      ) : (
        <div className="space-y-2">
          <AnimatePresence initial={false} mode="sync">
            {templates.map((template) => (
              <motion.div
                key={template.id}
                layout="position"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{
                  opacity: { duration: 0.14, ease: "easeOut" },
                  y: { duration: 0.18, ease: "easeOut" },
                  layout: { type: "spring", stiffness: 420, damping: 34, mass: 0.72 },
                }}
                className="rounded-xl border border-fg/10 bg-surface-el/30 transition active:bg-surface-el/50"
              >
                <button
                  type="button"
                  className="flex w-full items-start gap-3 p-3.5 text-left"
                  onClick={() =>
                    navigate(`/settings/characters/${characterId}/templates/${template.id}`)
                  }
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/5">
                    <MessageSquare className="h-4 w-4 text-fg/50" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <span className="truncate text-sm font-medium text-fg">{template.name}</span>
                    <p className="mt-0.5 text-xs text-fg/50">
                      {template.messages.length} message
                      {template.messages.length !== 1 ? "s" : ""}
                    </p>
                    {template.messages.length > 0 && (
                      <p className="mt-1 line-clamp-2 text-xs text-fg/40">
                        {template.messages[0].content.slice(0, 120)}
                        {template.messages[0].content.length > 120 ? "..." : ""}
                      </p>
                    )}
                  </div>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      setMenuTemplateId(template.id);
                    }}
                    className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-fg/40 transition active:bg-fg/10"
                  >
                    <MoreVertical className="h-4 w-4" />
                  </button>
                </button>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}

      {/* Context menu */}
      <BottomMenu
        isOpen={!!menuTemplateId}
        onClose={() => setMenuTemplateId(null)}
        title={menuTemplate?.name ?? "Template"}
      >
        <MenuButton
          icon={<Trash2 className="h-4 w-4" />}
          title="Delete template"
          color="from-rose-500 to-red-600"
          onClick={() => menuTemplateId && handleDelete(menuTemplateId)}
        />
      </BottomMenu>
    </motion.div>
  );
}
