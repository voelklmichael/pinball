use std::{fs, io::BufWriter, path::Path};

use anyhow::{Context, Result, bail};
use ts_formats::{bit, cmp};

fn usage() -> ! {
    eprintln!("usage:\n  ts-tools unpack <in> <out>\n  ts-tools bit2png <in.BIT> <out.png>");
    std::process::exit(2);
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["unpack", input, output] => {
            let data = cmp::unpack(&fs::read(input)?).context(input.to_owned())?;
            fs::write(output, data)?;
        }
        ["bit2png", input, output] => bit2png(Path::new(input), Path::new(output))?,
        _ => usage(),
    }
    Ok(())
}

fn bit2png(input: &Path, output: &Path) -> Result<()> {
    let data = cmp::unpack(&fs::read(input)?).with_context(|| input.display().to_string())?;
    let len = data.len();
    let Some(img) = bit::from_unpacked(data) else {
        bail!("{}: {len} bytes is not a known 24-bit screen size", input.display());
    };
    let mut enc = png::Encoder::new(BufWriter::new(fs::File::create(output)?), img.width, img.height);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&img.pixels)?;
    println!("{}: {}x{}", input.display(), img.width, img.height);
    Ok(())
}
