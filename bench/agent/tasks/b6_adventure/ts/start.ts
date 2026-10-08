export interface Room {
  readonly name: string;
  readonly exits: readonly (readonly [string, string])[];
  readonly items: readonly string[];
}

export interface Game {
  readonly room: string;
  readonly rooms: readonly Room[];
  readonly inventory: readonly string[];
  readonly moves: number;
}

export function room(name: string, exits: readonly (readonly [string, string])[], items: readonly string[]): Room {
  return { name, exits, items };
}

export function world(): Room[] {
  return [
    room("hall", [["north", "vault"], ["east", "kitchen"]], []),
    room("kitchen", [["west", "hall"]], ["key", "bread"]),
    room("vault", [["south", "hall"]], ["gold"]),
  ];
}

export function newGame(): Game {
  return { room: "hall", rooms: world(), inventory: [], moves: 0 };
}

export function current(g: Game): Room {
  const r = g.rooms.find((r) => r.name === g.room);
  if (r === undefined) throw new Error(`no room ${g.room}`);
  return r;
}

export function look(g: Game): string {
  const r = current(g);
  return `You are in the ${r.name}. Exits: ` + r.exits.map(([d]) => d).join(", ") + ".";
}

export function go(g: Game, dir: string): [Game, string] {
  for (const [d, target] of current(g).exits) {
    if (d === dir) {
      const g2 = { ...g, room: target, moves: g.moves + 1 };
      return [g2, look(g2)];
    }
  }
  return [g, "You can't go that way."];
}

export function step(g: Game, cmd: string): [Game, string] {
  const w = cmd.split(" ");
  if (w[0] === "look") return [g, look(g)];
  if (w[0] === "go" && w.length === 2) return go(g, w[1]);
  return [g, "I don't understand."];
}

export function run(cmds: readonly string[]): [Game, string[]] {
  let g = newGame();
  const out: string[] = [];
  for (const c of cmds) {
    const [g2, msg] = step(g, c);
    g = g2;
    out.push(msg);
  }
  return [g, out];
}

export function play(cmds: readonly string[]): string[] {
  return run(cmds)[1];
}
