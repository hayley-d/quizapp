import {
	type ChangeEvent,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { Plus, Upload } from "lucide-react";
import { toast } from "sonner";
import {
	ApiError,
	api,
	type Deck,
	type DeckSort,
	type Module,
	type ModuleFilter,
	type TransferImportResult,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { ModuleDialog } from "@/components/ModuleDialog";
import { DeckDialog } from "@/components/DeckDialog";
import { DeckCard } from "@/components/DeckCard";

const ALL = "all";
const NONE = "none";

export function DecksPage() {
	const [modules, setModules] = useState<Module[]>([]);
	const [decks, setDecks] = useState<Deck[]>([]);
	const [editing, setEditing] = useState<Deck | "new" | null>(null);

	const [search, setSearch] = useState("");
	const [debouncedSearch, setDebouncedSearch] = useState("");
	const [moduleFilter, setModuleFilter] = useState<ModuleFilter>(ALL);
	const [sort, setSort] = useState<DeckSort>("newest");
	const [loading, setLoading] = useState(false);
	const [importing, setImporting] = useState(false);

	const filtersActive = debouncedSearch.trim() !== "" || moduleFilter !== ALL;

	useEffect(() => {
		const timeoutId = setTimeout(() => setDebouncedSearch(search), 250);
		return () => clearTimeout(timeoutId);
	}, [search]);

	const loadModules = useCallback(async () => {
		try {
			setModules(await api.listModules());
		} catch {
			toast.error("Could not load modules");
		}
	}, []);

	useEffect(() => {
		void loadModules();
	}, [loadModules]);

	const inFlight = useRef<AbortController | null>(null);

	const loadDecks = useCallback(async () => {
		inFlight.current?.abort();
		const controller = new AbortController();
		inFlight.current = controller;
		setLoading(true);
		try {
			const rows = await api.listDecks(
				{ search: debouncedSearch, moduleId: moduleFilter, sort },
				controller.signal,
			);
			setDecks(rows);
		} catch (error) {
			if ((error as Error)?.name === "AbortError") return;
			toast.error("Could not load decks");
		} finally {
			if (inFlight.current === controller) setLoading(false);
		}
	}, [debouncedSearch, moduleFilter, sort]);

	useEffect(() => {
		void loadDecks();
	}, [loadDecks]);

	useEffect(() => () => inFlight.current?.abort(), []);

	function clearFilters() {
		setSearch("");
		setDebouncedSearch("");
		setModuleFilter(ALL);
	}

	function describeImport(result: TransferImportResult) {
		const deckCount = result.decks.length;
		const cardCount = result.decks.reduce(
			(total, deck) => total + deck.card_count,
			0,
		);
		const parts = [
			`${deckCount} ${deckCount === 1 ? "deck" : "decks"}`,
			`${cardCount} ${cardCount === 1 ? "card" : "cards"}`,
		];
		if (result.image_count > 0) {
			parts.push(
				`${result.image_count} ${result.image_count === 1 ? "image" : "images"}`,
			);
		}
		const renamed = result.decks.filter(
			(deck) => deck.name !== deck.original_name,
		);
		const description = renamed.length
			? renamed
					.map((deck) => `“${deck.original_name}” came in as “${deck.name}”`)
					.join("; ")
			: undefined;
		return { message: `Imported ${parts.join(", ")}`, description };
	}

	async function importFile(event: ChangeEvent<HTMLInputElement>) {
		const file = event.target.files?.[0];
		event.target.value = "";
		if (!file) return;

		setImporting(true);
		try {
			const result = await api.importTransfer(file);
			const { message, description } = describeImport(result);
			toast.success(message, { description });
			void loadModules();
			void loadDecks();
		} catch (error) {
			toast.error(
				error instanceof ApiError ? error.message : "Could not import that file",
			);
		} finally {
			setImporting(false);
		}
	}

	const moduleName = (filter: ModuleFilter) =>
		filter === ALL
			? "All modules"
			: filter === NONE
				? "No module"
				: (modules.find((module) => module.id === filter)?.name ??
					"Unknown module");

	return (
		<div className="space-y-6">
			<div className="flex flex-wrap items-center justify-between gap-3">
				<h1 className="font-display text-2xl font-bold">Decks</h1>
				<div className="flex gap-2">
					<input
						id="import-transfer-file"
						type="file"
						className="sr-only"
						accept=".json,application/json"
						disabled={importing}
						onChange={(event) => void importFile(event)}
					/>
					<Button
						asChild
						variant="outline"
						className={`h-10 px-4 ${importing ? "pointer-events-none opacity-50" : ""}`}
					>
						<label htmlFor="import-transfer-file">
							<Upload className="size-4" />
							{importing ? "Importing…" : "Import"}
						</label>
					</Button>
					<ModuleDialog
						modules={modules}
						onChanged={(deletedModuleId) => {
							if (
								deletedModuleId !== null &&
								moduleFilter === deletedModuleId
							) {
								setModuleFilter(ALL);
							}
							void loadModules();
							void loadDecks();
						}}
					/>
					<Button
						className="h-10 px-4"
						onClick={() => setEditing("new")}
					>
						<Plus className="size-4" />
						Create deck
					</Button>
				</div>
			</div>

			<div className="flex flex-col gap-2 rounded-xl border bg-card p-3 shadow-sm sm:flex-row sm:items-center">
				<Input
					className="h-10 min-w-0 flex-1"
					placeholder="Search deck names…"
					value={search}
					onChange={(event) => setSearch(event.target.value)}
				/>
				<Select
					value={String(moduleFilter)}
					onValueChange={(selectedValue) =>
						setModuleFilter(
							selectedValue === ALL || selectedValue === NONE
								? (selectedValue as ModuleFilter)
								: Number(selectedValue),
						)
					}
				>
					<SelectTrigger className="data-[size=default]:h-10 sm:w-52">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						<SelectItem value={ALL}>All modules</SelectItem>
						<SelectItem value={NONE}>No module</SelectItem>
						{modules.map((module) => (
							<SelectItem
								key={module.id}
								value={String(module.id)}
							>
								{module.name}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
				<Select
					value={sort}
					onValueChange={(selectedValue) =>
						setSort(selectedValue as DeckSort)
					}
				>
					<SelectTrigger className="data-[size=default]:h-10 sm:w-44">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						<SelectItem value="newest">Newest first</SelectItem>
						<SelectItem value="oldest">Oldest first</SelectItem>
					</SelectContent>
				</Select>
			</div>

			{decks.length === 0 &&
				!loading &&
				(filtersActive ? (
					<div className="space-y-2">
						<p className="text-muted-foreground">
							No decks match “{debouncedSearch}” in{" "}
							{moduleName(moduleFilter)}.
						</p>
						<Button
							variant="secondary"
							size="sm"
							onClick={clearFilters}
						>
							Clear filters
						</Button>
					</div>
				) : (
					<p className="text-muted-foreground">No decks yet.</p>
				))}

			<div className="grid gap-4 lg:grid-cols-2">
				{decks.map((deck) => (
					<DeckCard
						key={deck.id}
						deck={deck}
						onEdit={() => setEditing(deck)}
						onFilterModule={setModuleFilter}
					/>
				))}
			</div>

			{editing && (
				<DeckDialog
					key={editing === "new" ? "new" : editing.id}
					modules={modules}
					deck={editing === "new" ? undefined : editing}
					open
					onOpenChange={(isOpen) => {
						if (!isOpen) setEditing(null);
					}}
					onSaved={() => {
						void loadDecks();
						void loadModules();
					}}
					onDeleted={() => {
						void loadDecks();
						void loadModules();
					}}
				/>
			)}
		</div>
	);
}
