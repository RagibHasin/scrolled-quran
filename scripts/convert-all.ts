const ayahs_json: { [id: string]: { text: string } } = JSON.parse(
  await Deno.readTextFile("../assets/digital-khatt-v2.aba.json"),
);

const basmalah = Object.values(ayahs_json)[0]
  .text.split(" ")
  .slice(0, -1)
  .join(" ");
const ayahs = Object.values(ayahs_json)
  .map(({ text }) => `"${text}"`)
  .join(",\n    ");

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

const fileText = `use crate::model::{PlaceOfRevelation::*, SurahInfo};

pub const AYAHS: [&str; 6237] = [
    "${basmalah} ",
    ${ayahs},
];

pub const SURAHS: [SurahInfo; 115] = [
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
