import { json } from "solid-start/api";
import process from "node:process";

export type StatusResp = Array<{
  name: string;
  status: "starting" | "online";
}>;

const endpoint = process.env["ENDPOINT"] || "http://127.0.0.1:25580/status";
console.log("status get endpoint", endpoint);

export async function GET() {
  try {
    const resp = await fetch(endpoint);
    const statusResp = await resp.text();

    // Don't wanna use JSON on the Rust side of things when this is... okay, I guess...
    const computers = statusResp
      .split("\n")
      .map((line) => line.split(","))
      .map(([name, status]) => ({ name, status }));

    return json(computers);
  } catch (err) {
    console.error("Error fetching", err);
    throw err;
  }
}
