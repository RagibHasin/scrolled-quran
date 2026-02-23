import { DatabaseSync } from "node:sqlite";

const dbWords = new DatabaseSync("../assets/digital-khatt-v2.db");
const selectWord = dbWords.prepare(`SELECT surah, ayah FROM words WHERE id=?;`);

const dbPages = new DatabaseSync("../assets/digital-khatt-15-lines.db");

const basmalah = dbWords
  .prepare(`SELECT text FROM words WHERE surah=1 AND ayah=1 ORDER BY id;`)
  .all()
  .map(({ text }) => text as string)
  .join(" ")
  .slice(0, -1);

const { text: ayahs } = dbWords
  .prepare(`SELECT surah, ayah, text FROM words ORDER BY id;`)
  .iterate()
  .map((r) => r as { surah: number; ayah: number; text: string })
  .reduce(
    (
      { surah: last_surah, ayah: last_ayah, text: acc },
      { surah, ayah, text },
    ) => {
      const sep =
        last_surah === (surah as number) && last_ayah === (ayah as number)
          ? " "
          : `",\n    "`;
      return { surah, ayah, text: acc + sep + text };
    },
  );

const surahs_json: {
  [id: string]: {
    name: string;
    name_simple: string;
    name_arabic: string;
    revelation_order: string;
    revelation_place: string;
    verses_count: number;
  };
} = JSON.parse(
  await Deno.readTextFile("../assets/quran-metadata-surah-name.json"),
);

let cumulative_ayahs = 0;

const surahs = Object.values(surahs_json)
  .map((surah) => {
    const revealed_in =
      surah.revelation_place === "madinah" ? "Madinah" : "Makkah";
    const info = `SurahInfo {
        name_ar: "${surah.name_arabic}",
        name_en: "${surah.name}",
        name_en_simple: "${surah.name_simple}",
        ayahs: ${surah.verses_count},
        cumulative_ayahs: ${cumulative_ayahs},
        revealed_in: ${revealed_in},
    }`;
    cumulative_ayahs += surah.verses_count;
    return info;
  })
  .join(",\n    ");

const first_ayahs = dbPages
  .prepare(
    `SELECT page_number, MIN(line_number), first_word_id FROM pages WHERE line_type='ayah' GROUP BY page_number ORDER BY page_number;`,
  )
  .iterate()
  .map(
    ({ first_word_id }) =>
      selectWord.get(first_word_id) as { surah: number; ayah: number },
  )
  .map(({ surah, ayah }) => `(${surah}, ${ayah})`)
  .reduce((acc, v) => acc + ",\n    " + v);

dbWords.close();
dbPages.close();

const fileText = `use crate::model::{PlaceOfRevelation::*, SurahInfo};

pub static AYAHS: [&str; 6237] = [
    "${basmalah} ",
    "${ayahs}",
];

pub static FIRST_AYAHS: [(u8, u16); 604] = [
    ${first_ayahs},
];

pub static SURAHS: [SurahInfo; 115] = [
    SurahInfo {
        name_ar: "",
        name_en: "",
        name_en_simple: "",
        ayahs: 0,
        cumulative_ayahs: 0,
        revealed_in: Makkah,
    },
    ${surahs},
];
`;

await Deno.writeTextFile("../src/data.rs", fileText);
