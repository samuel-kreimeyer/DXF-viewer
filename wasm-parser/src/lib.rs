use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{Mutex, OnceLock};

static RESULT_BUF: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

fn result_buf() -> &'static Mutex<Vec<u8>> {
    RESULT_BUF.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Clone)]
struct Entity {
    kind: &'static str,
    vertices: Vec<Point>,
    layer: Option<String>,
    text: Option<String>,
    text_height: Option<f64>,
}

struct BlockDefinition {
    base: Point,
    entities: Vec<Entity>,
}

#[derive(Clone)]
struct Pair {
    code: i32,
    value: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut data = Vec::<u8>::with_capacity(size);
    let ptr = data.as_mut_ptr();
    std::mem::forget(data);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: *mut u8, capacity: usize) {
    if ptr.is_null() || capacity == 0 {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr, 0, capacity));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn parse_dxf(ptr: *const u8, len: usize) -> u32 {
    if ptr.is_null() || len == 0 {
        write_result_json(error_json("Input file is empty."));
        return 0;
    }

    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = match std::str::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => {
            write_result_json(error_json("DXF parsing expects UTF-8/ASCII text input."));
            return 0;
        }
    };

    match parse_entities_from_text(text) {
        Ok((entities, warnings)) => {
            write_result_json(success_json(&entities, &warnings));
            1
        }
        Err(message) => {
            write_result_json(error_json(&message));
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn result_ptr() -> *const u8 {
    let guard = result_buf().lock().expect("result mutex poisoned");
    guard.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn result_len() -> usize {
    let guard = result_buf().lock().expect("result mutex poisoned");
    guard.len()
}

fn write_result_json(json: String) {
    let mut guard = result_buf().lock().expect("result mutex poisoned");
    guard.clear();
    guard.extend_from_slice(json.as_bytes());
}

fn parse_entities_from_text(text: &str) -> Result<(Vec<Entity>, Vec<String>), String> {
    let pairs = parse_pairs(text)?;
    let mut entities = Vec::<Entity>::new();
    let mut warnings = Vec::<String>::new();
    let mut unsupported = BTreeMap::<String, usize>::new();
    let (blocks, mut block_warnings) = parse_block_definitions(&pairs);
    let mut in_entities = false;
    let mut i = 0_usize;

    while i < pairs.len() {
        let pair = &pairs[i];

        if pair.code == 0 && pair.value.eq_ignore_ascii_case("SECTION") {
            if let Some(next) = pairs.get(i + 1)
                && next.code == 2
                && next.value.eq_ignore_ascii_case("ENTITIES")
            {
                in_entities = true;
                i += 2;
                continue;
            }
        }

        if pair.code == 0 && pair.value.eq_ignore_ascii_case("ENDSEC") {
            in_entities = false;
            i += 1;
            continue;
        }

        if in_entities && pair.code == 0 {
            let kind = pair.value.to_ascii_uppercase();
            let mut end = i + 1;
            while end < pairs.len() && pairs[end].code != 0 {
                end += 1;
            }
            let payload = &pairs[(i + 1)..end];

            match kind.as_str() {
                "POINT" => {
                    if let Some(entity) = parse_point(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed POINT entity.".to_string());
                    }
                }
                "LINE" => {
                    if let Some(entity) = parse_line(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed LINE entity.".to_string());
                    }
                }
                "LWPOLYLINE" => {
                    if let Some(entity) = parse_lwpolyline(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed LWPOLYLINE entity.".to_string());
                    }
                }
                "CIRCLE" => {
                    if let Some(entity) = parse_circle(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed CIRCLE entity.".to_string());
                    }
                }
                "ARC" => {
                    if let Some(entity) = parse_arc(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed ARC entity.".to_string());
                    }
                }
                "ELLIPSE" => {
                    if let Some(entity) = parse_ellipse(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed ELLIPSE entity.".to_string());
                    }
                }
                "SPLINE" => {
                    if let Some(entity) = parse_spline(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed SPLINE entity.".to_string());
                    }
                }
                "3DFACE" => {
                    if let Some(entity) = parse_3dface(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed 3DFACE entity.".to_string());
                    }
                }
                "SOLID" => {
                    if let Some(entity) = parse_solid_or_trace(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed SOLID entity.".to_string());
                    }
                }
                "TRACE" => {
                    if let Some(entity) = parse_solid_or_trace(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed TRACE entity.".to_string());
                    }
                }
                "HATCH" => {
                    let (entity, partial) = parse_hatch(payload);
                    if let Some(entity) = entity {
                        entities.push(entity);
                        if partial {
                            warnings.push(
                                "HATCH entity contains unsupported boundary edges; rendered partial boundary."
                                    .to_string(),
                            );
                        }
                    } else {
                        warnings.push("Skipped malformed HATCH entity.".to_string());
                    }
                }
                "INSERT" => {
                    let (insert_entities, insert_warning) = parse_insert(payload, &blocks);
                    if let Some(warning) = insert_warning {
                        warnings.push(warning);
                    }
                    if let Some(insert_entities) = insert_entities {
                        entities.extend(insert_entities);
                    } else {
                        warnings.push("Skipped malformed INSERT entity.".to_string());
                    }
                }
                "DIMENSION" => {
                    let (dimension_entities, dimension_warning) = parse_dimension(payload, &blocks);
                    if let Some(warning) = dimension_warning {
                        warnings.push(warning);
                    }
                    if let Some(dimension_entities) = dimension_entities {
                        entities.extend(dimension_entities);
                    } else {
                        warnings.push("Skipped malformed DIMENSION entity.".to_string());
                    }
                }
                "XLINE" => {
                    if let Some(entity) = parse_xline(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed XLINE entity.".to_string());
                    }
                }
                "RAY" => {
                    if let Some(entity) = parse_ray(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed RAY entity.".to_string());
                    }
                }
                "LEADER" => {
                    if let Some(leader_entities) = parse_leader(payload) {
                        entities.extend(leader_entities);
                    } else {
                        warnings.push("Skipped malformed LEADER entity.".to_string());
                    }
                }
                "MULTILEADER" | "MLEADER" => {
                    if let Some(multileader_entities) = parse_multileader(payload) {
                        entities.extend(multileader_entities);
                    } else {
                        warnings.push("Skipped malformed MULTILEADER entity.".to_string());
                    }
                }
                "TEXT" => {
                    if let Some(entity) = parse_text(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed TEXT entity.".to_string());
                    }
                }
                "MTEXT" => {
                    if let Some(entity) = parse_mtext(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed MTEXT entity.".to_string());
                    }
                }
                "ATTDEF" => {
                    if let Some(entity) = parse_attribute_text(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed ATTDEF entity.".to_string());
                    }
                }
                "ATTRIB" => {
                    if let Some(entity) = parse_attribute_text(payload) {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed ATTRIB entity.".to_string());
                    }
                }
                "POLYLINE" => {
                    let (entity, next_index) = parse_polyline_sequence(&pairs, i);
                    i = next_index;
                    if let Some(entity) = entity {
                        entities.push(entity);
                    } else {
                        warnings.push("Skipped malformed POLYLINE entity.".to_string());
                    }
                    continue;
                }
                "ENDSEC" => in_entities = false,
                "SEQEND" | "VERTEX" => {
                    *unsupported.entry(kind).or_insert(0) += 1;
                }
                _ => {
                    *unsupported.entry(kind).or_insert(0) += 1;
                }
            }

            i = end;
            continue;
        }

        i += 1;
    }

    for (kind, count) in unsupported {
        warnings.push(format!(
            "Unsupported entity type {kind} encountered {count} time(s)."
        ));
    }
    warnings.append(&mut block_warnings);

    if entities.is_empty() {
        warnings.push("No renderable entities were found in ENTITIES section.".to_string());
    }

    Ok((entities, warnings))
}

fn parse_block_definitions(pairs: &[Pair]) -> (BTreeMap<String, BlockDefinition>, Vec<String>) {
    let mut blocks = BTreeMap::<String, BlockDefinition>::new();
    let mut warnings = Vec::<String>::new();
    let mut in_blocks = false;
    let mut i = 0_usize;

    while i < pairs.len() {
        let pair = &pairs[i];

        if pair.code == 0 && pair.value.eq_ignore_ascii_case("SECTION") {
            if let Some(next) = pairs.get(i + 1)
                && next.code == 2
                && next.value.eq_ignore_ascii_case("BLOCKS")
            {
                in_blocks = true;
                i += 2;
                continue;
            }
        }

        if in_blocks && pair.code == 0 && pair.value.eq_ignore_ascii_case("ENDSEC") {
            in_blocks = false;
            i += 1;
            continue;
        }

        if !in_blocks {
            i += 1;
            continue;
        }

        if pair.code == 0 && pair.value.eq_ignore_ascii_case("BLOCK") {
            let mut header_end = i + 1;
            while header_end < pairs.len() && pairs[header_end].code != 0 {
                header_end += 1;
            }
            let header = &pairs[(i + 1)..header_end];

            let mut name = None;
            let mut base_x = Some(0.0);
            let mut base_y = Some(0.0);
            let mut base_z = Some(0.0);
            for h in header {
                match h.code {
                    2 => name = Some(h.value.clone()),
                    10 => base_x = parse_f64(&h.value),
                    20 => base_y = parse_f64(&h.value),
                    30 => base_z = parse_f64(&h.value),
                    _ => {}
                }
            }

            let mut cursor = header_end;
            let mut entities = Vec::<Entity>::new();
            while cursor < pairs.len() {
                let marker = &pairs[cursor];
                if marker.code == 0 && marker.value.eq_ignore_ascii_case("ENDBLK") {
                    cursor += 1;
                    break;
                }
                if marker.code != 0 {
                    cursor += 1;
                    continue;
                }

                let kind = marker.value.to_ascii_uppercase();
                let mut end = cursor + 1;
                while end < pairs.len() && pairs[end].code != 0 {
                    end += 1;
                }
                let payload = &pairs[(cursor + 1)..end];

                match kind.as_str() {
                    "POINT" => {
                        if let Some(entity) = parse_point(payload) {
                            entities.push(entity);
                        }
                    }
                    "LINE" => {
                        if let Some(entity) = parse_line(payload) {
                            entities.push(entity);
                        }
                    }
                    "LWPOLYLINE" => {
                        if let Some(entity) = parse_lwpolyline(payload) {
                            entities.push(entity);
                        }
                    }
                    "CIRCLE" => {
                        if let Some(entity) = parse_circle(payload) {
                            entities.push(entity);
                        }
                    }
                    "ARC" => {
                        if let Some(entity) = parse_arc(payload) {
                            entities.push(entity);
                        }
                    }
                    "ELLIPSE" => {
                        if let Some(entity) = parse_ellipse(payload) {
                            entities.push(entity);
                        }
                    }
                    "SPLINE" => {
                        if let Some(entity) = parse_spline(payload) {
                            entities.push(entity);
                        }
                    }
                    "3DFACE" => {
                        if let Some(entity) = parse_3dface(payload) {
                            entities.push(entity);
                        }
                    }
                    "SOLID" | "TRACE" => {
                        if let Some(entity) = parse_solid_or_trace(payload) {
                            entities.push(entity);
                        }
                    }
                    "HATCH" => {
                        let (entity, partial) = parse_hatch(payload);
                        if let Some(entity) = entity {
                            entities.push(entity);
                            if partial {
                                warnings.push(format!(
                                    "BLOCK {} contains partial HATCH boundary rendering.",
                                    name.clone().unwrap_or_else(|| "<unnamed>".to_string())
                                ));
                            }
                        }
                    }
                    "TEXT" => {
                        if let Some(entity) = parse_text(payload) {
                            entities.push(entity);
                        }
                    }
                    "MTEXT" => {
                        if let Some(entity) = parse_mtext(payload) {
                            entities.push(entity);
                        }
                    }
                    "ATTDEF" | "ATTRIB" => {
                        if let Some(entity) = parse_attribute_text(payload) {
                            entities.push(entity);
                        }
                    }
                    "XLINE" => {
                        if let Some(entity) = parse_xline(payload) {
                            entities.push(entity);
                        }
                    }
                    "RAY" => {
                        if let Some(entity) = parse_ray(payload) {
                            entities.push(entity);
                        }
                    }
                    "LEADER" => {
                        if let Some(leader_entities) = parse_leader(payload) {
                            entities.extend(leader_entities);
                        }
                    }
                    "MULTILEADER" | "MLEADER" => {
                        if let Some(multileader_entities) = parse_multileader(payload) {
                            entities.extend(multileader_entities);
                        }
                    }
                    "POLYLINE" => {
                        let (entity, next_index) = parse_polyline_sequence(pairs, cursor);
                        cursor = next_index;
                        if let Some(entity) = entity {
                            entities.push(entity);
                        }
                        continue;
                    }
                    "INSERT" => {
                        warnings.push(format!(
                            "Nested INSERT in BLOCK {} is not expanded.",
                            name.clone().unwrap_or_else(|| "<unnamed>".to_string())
                        ));
                    }
                    _ => {}
                }

                cursor = end;
            }

            if let Some(name) = name {
                blocks.insert(
                    name,
                    BlockDefinition {
                        base: Point {
                            x: base_x.unwrap_or(0.0),
                            y: base_y.unwrap_or(0.0),
                            z: base_z.unwrap_or(0.0),
                        },
                        entities,
                    },
                );
            } else {
                warnings.push("Encountered BLOCK without a valid name.".to_string());
            }

            i = cursor;
            continue;
        }

        i += 1;
    }

    (blocks, warnings)
}

fn parse_polyline_sequence(pairs: &[Pair], start_index: usize) -> (Option<Entity>, usize) {
    let mut cursor = start_index + 1;
    let mut layer = None;
    let mut closed = false;
    let mut header_flags = 0_i32;

    // Consume POLYLINE header payload.
    while cursor < pairs.len() && pairs[cursor].code != 0 {
        let pair = &pairs[cursor];
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            70 => {
                if let Ok(flag) = pair.value.parse::<i32>() {
                    header_flags = flag;
                    closed = (flag & 1) == 1 || (flag & 32) == 32;
                }
            }
            _ => {}
        }
        cursor += 1;
    }

    let mut points = Vec::<Point>::new();
    let mut saw_seqend = false;

    while cursor < pairs.len() && pairs[cursor].code == 0 {
        let marker = pairs[cursor].value.to_ascii_uppercase();

        if marker == "VERTEX" {
            let mut end = cursor + 1;
            while end < pairs.len() && pairs[end].code != 0 {
                end += 1;
            }
            if let Some((point, is_face_record)) =
                parse_vertex_payload(&pairs[(cursor + 1)..end], header_flags)
                && !is_face_record
            {
                points.push(point);
            }
            cursor = end;
            continue;
        }

        if marker == "SEQEND" {
            saw_seqend = true;
            cursor += 1;
            break;
        }

        // Reached another entity marker without SEQEND.
        break;
    }

    if !saw_seqend {
        return (None, cursor);
    }

    if closed
        && points.len() > 2
        && let Some(first) = points.first().copied()
    {
        points.push(first);
    }

    if points.len() < 2 {
        return (None, cursor);
    }

    (
        Some(Entity {
            kind: "polyline",
            vertices: points,
            layer,
            text: None,
            text_height: None,
        }),
        cursor,
    )
}

fn parse_vertex_payload(payload: &[Pair], header_flags: i32) -> Option<(Point, bool)> {
    let mut x = None;
    let mut y = None;
    let mut z = Some(0.0);
    let mut vertex_flags = 0_i32;

    for pair in payload {
        match pair.code {
            10 => x = parse_f64(&pair.value),
            20 => y = parse_f64(&pair.value),
            30 => z = parse_f64(&pair.value),
            70 => {
                if let Ok(flag) = pair.value.parse::<i32>() {
                    vertex_flags = flag;
                }
            }
            _ => {}
        }
    }

    // Polyface mesh face-record vertices are index records, not render coordinates.
    let is_polyface = (header_flags & 64) == 64;
    let is_face_record = is_polyface && (vertex_flags & 128) == 128;

    Some((
        Point {
            x: x?,
            y: y?,
            z: z.unwrap_or(0.0),
        },
        is_face_record,
    ))
}

fn parse_pairs(text: &str) -> Result<Vec<Pair>, String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();

    if lines.len() < 2 {
        return Err("DXF input is too short to parse group code pairs.".to_string());
    }

    let mut pairs = Vec::<Pair>::new();
    let mut i = 0_usize;

    while i + 1 < lines.len() {
        let code_raw = lines[i].trim();
        let value = lines[i + 1].trim().to_string();

        let code = code_raw
            .parse::<i32>()
            .map_err(|_| format!("Invalid group code '{code_raw}' at line {}.", i + 1))?;

        pairs.push(Pair { code, value });
        i += 2;
    }

    Ok(pairs)
}

fn parse_point(payload: &[Pair]) -> Option<Entity> {
    let x = value_for_code(payload, 10)?;
    let y = value_for_code(payload, 20)?;
    let z = value_for_code(payload, 30).unwrap_or(0.0);
    let layer = payload
        .iter()
        .find(|pair| pair.code == 8)
        .map(|pair| pair.value.clone());

    // Fallback render as a tiny cross-arm line so current viewer draws a visible marker.
    let marker_half = 0.25;
    Some(Entity {
        kind: "line",
        vertices: vec![
            Point {
                x: x - marker_half,
                y,
                z,
            },
            Point {
                x: x + marker_half,
                y,
                z,
            },
        ],
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_line(payload: &[Pair]) -> Option<Entity> {
    let mut x1 = None;
    let mut y1 = None;
    let mut z1 = Some(0.0);
    let mut x2 = None;
    let mut y2 = None;
    let mut z2 = Some(0.0);
    let mut layer = None;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => x1 = parse_f64(&pair.value),
            20 => y1 = parse_f64(&pair.value),
            30 => z1 = parse_f64(&pair.value),
            11 => x2 = parse_f64(&pair.value),
            21 => y2 = parse_f64(&pair.value),
            31 => z2 = parse_f64(&pair.value),
            _ => {}
        }
    }

    Some(Entity {
        kind: "line",
        vertices: vec![
            Point {
                x: x1?,
                y: y1?,
                z: z1.unwrap_or(0.0),
            },
            Point {
                x: x2?,
                y: y2?,
                z: z2.unwrap_or(0.0),
            },
        ],
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_lwpolyline(payload: &[Pair]) -> Option<Entity> {
    let mut points = Vec::<Point>::new();
    let mut pending_x = None;
    let mut elevation = 0.0;
    let mut layer = None;
    let mut closed = false;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => pending_x = parse_f64(&pair.value),
            20 => {
                if let (Some(x), Some(y)) = (pending_x.take(), parse_f64(&pair.value)) {
                    points.push(Point { x, y, z: elevation });
                }
            }
            38 => {
                if let Some(z) = parse_f64(&pair.value) {
                    elevation = z;
                }
            }
            70 => {
                if let Ok(flag) = pair.value.parse::<i32>() {
                    closed = (flag & 1) == 1;
                }
            }
            _ => {}
        }
    }

    if closed
        && points.len() > 2
        && let Some(first) = points.first().copied()
    {
        points.push(first);
    }

    if points.len() < 2 {
        return None;
    }

    Some(Entity {
        kind: "polyline",
        vertices: points,
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_3dface(payload: &[Pair]) -> Option<Entity> {
    let layer = payload
        .iter()
        .find(|pair| pair.code == 8)
        .map(|pair| pair.value.clone());

    let p1 = point_for_codes(payload, 10, 20, 30)?;
    let p2 = point_for_codes(payload, 11, 21, 31)?;
    let p3 = point_for_codes(payload, 12, 22, 32)?;
    let p4 = point_for_codes(payload, 13, 23, 33).unwrap_or(p3);

    let mut outline = vec![p1, p2, p3];
    if !points_approx_equal(p3, p4) {
        outline.push(p4);
    }
    if let Some(first) = outline.first().copied() {
        outline.push(first);
    }
    if outline.len() < 4 {
        return None;
    }

    Some(Entity {
        kind: "polyline",
        vertices: outline,
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_solid_or_trace(payload: &[Pair]) -> Option<Entity> {
    let layer = payload
        .iter()
        .find(|pair| pair.code == 8)
        .map(|pair| pair.value.clone());

    let p1 = point_for_codes(payload, 10, 20, 30)?;
    let p2 = point_for_codes(payload, 11, 21, 31)?;
    let p3 = point_for_codes(payload, 12, 22, 32)?;
    let p4 = point_for_codes(payload, 13, 23, 33)?;

    let mut outline = vec![p1, p2, p4, p3];
    if let Some(first) = outline.first().copied() {
        outline.push(first);
    }

    Some(Entity {
        kind: "polyline",
        vertices: outline,
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_circle(payload: &[Pair]) -> Option<Entity> {
    let mut cx = None;
    let mut cy = None;
    let mut cz = Some(0.0);
    let mut radius = None;
    let mut layer = None;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => cx = parse_f64(&pair.value),
            20 => cy = parse_f64(&pair.value),
            30 => cz = parse_f64(&pair.value),
            40 => radius = parse_f64(&pair.value),
            _ => {}
        }
    }

    let center = Point {
        x: cx?,
        y: cy?,
        z: cz.unwrap_or(0.0),
    };
    let vertices = sample_arc(center, radius?, 0.0, 360.0, 48);

    Some(Entity {
        kind: "polyline",
        vertices,
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_arc(payload: &[Pair]) -> Option<Entity> {
    let mut cx = None;
    let mut cy = None;
    let mut cz = Some(0.0);
    let mut radius = None;
    let mut start = None;
    let mut end = None;
    let mut layer = None;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => cx = parse_f64(&pair.value),
            20 => cy = parse_f64(&pair.value),
            30 => cz = parse_f64(&pair.value),
            40 => radius = parse_f64(&pair.value),
            50 => start = parse_f64(&pair.value),
            51 => end = parse_f64(&pair.value),
            _ => {}
        }
    }

    let center = Point {
        x: cx?,
        y: cy?,
        z: cz.unwrap_or(0.0),
    };
    let start_angle = start?;
    let mut end_angle = end?;

    if end_angle <= start_angle {
        end_angle += 360.0;
    }

    let span = (end_angle - start_angle).max(1.0);
    let segments = ((span / 360.0) * 48.0).ceil().max(8.0) as usize;
    let vertices = sample_arc(center, radius?, start_angle, end_angle, segments);

    Some(Entity {
        kind: "polyline",
        vertices,
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_ellipse(payload: &[Pair]) -> Option<Entity> {
    let mut cx = None;
    let mut cy = None;
    let mut cz = Some(0.0);
    let mut major_x = None;
    let mut major_y = None;
    let mut ratio = None;
    let mut start_param = Some(0.0);
    let mut end_param = Some(std::f64::consts::TAU);
    let mut layer = None;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => cx = parse_f64(&pair.value),
            20 => cy = parse_f64(&pair.value),
            30 => cz = parse_f64(&pair.value),
            11 => major_x = parse_f64(&pair.value),
            21 => major_y = parse_f64(&pair.value),
            40 => ratio = parse_f64(&pair.value),
            41 => start_param = parse_f64(&pair.value),
            42 => end_param = parse_f64(&pair.value),
            _ => {}
        }
    }

    let center = Point {
        x: cx?,
        y: cy?,
        z: cz.unwrap_or(0.0),
    };
    let major_axis_x = major_x?;
    let major_axis_y = major_y?;
    let major_len = major_axis_x.hypot(major_axis_y);
    if major_len <= 1e-9 {
        return None;
    }

    let minor_ratio = ratio?.abs();
    if !minor_ratio.is_finite() || minor_ratio <= 1e-9 {
        return None;
    }

    let ux = major_axis_x / major_len;
    let uy = major_axis_y / major_len;
    let minor_axis_x = -uy * major_len * minor_ratio;
    let minor_axis_y = ux * major_len * minor_ratio;

    let start = start_param?;
    let mut end = end_param?;
    if end <= start {
        end += std::f64::consts::TAU;
    }
    let span = (end - start).max(0.01);
    let segments = ((span / std::f64::consts::TAU) * 96.0)
        .ceil()
        .clamp(12.0, 192.0) as usize;
    let vertices = sample_ellipse(
        center,
        major_axis_x,
        major_axis_y,
        minor_axis_x,
        minor_axis_y,
        start,
        end,
        segments,
    );

    Some(Entity {
        kind: "polyline",
        vertices,
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_spline(payload: &[Pair]) -> Option<Entity> {
    let mut layer = None;
    let mut closed = false;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            70 => {
                if let Ok(flag) = pair.value.parse::<i32>() {
                    closed = (flag & 1) == 1;
                }
            }
            _ => {}
        }
    }

    let fit_points = collect_point_sequence(payload, 11, 21, 31);
    let mut vertices = if fit_points.len() >= 2 {
        fit_points
    } else {
        collect_point_sequence(payload, 10, 20, 30)
    };

    if closed
        && vertices.len() > 2
        && let Some(first) = vertices.first().copied()
    {
        vertices.push(first);
    }

    if vertices.len() < 2 {
        return None;
    }

    Some(Entity {
        kind: "polyline",
        vertices,
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_hatch(payload: &[Pair]) -> (Option<Entity>, bool) {
    let mut layer = None;
    for pair in payload {
        if pair.code == 8 {
            layer = Some(pair.value.clone());
            break;
        }
    }

    let mut best_vertices: Option<Vec<Point>> = None;
    let mut partial = false;
    let mut i = 0_usize;

    while i < payload.len() {
        if payload[i].code != 92 {
            i += 1;
            continue;
        }

        let path_flags = payload[i].value.parse::<i32>().unwrap_or(0);
        let start = i + 1;
        let mut end = start;
        while end < payload.len() && payload[end].code != 92 {
            end += 1;
        }
        let segment = &payload[start..end];

        let (vertices, segment_partial) = if (path_flags & 2) == 2 {
            (parse_hatch_polyline_path(segment), false)
        } else {
            parse_hatch_edge_path(segment)
        };

        partial |= segment_partial;

        if let Some(vertices) = vertices
            && vertices.len() >= 2
            && best_vertices
                .as_ref()
                .map(|existing| vertices.len() > existing.len())
                .unwrap_or(true)
        {
            best_vertices = Some(vertices);
        }

        i = end;
    }

    let Some(vertices) = best_vertices else {
        return (None, partial);
    };

    (
        Some(Entity {
            kind: "polyline",
            vertices,
            layer,
            text: None,
            text_height: None,
        }),
        partial,
    )
}

fn parse_hatch_polyline_path(segment: &[Pair]) -> Option<Vec<Point>> {
    let mut vertices = Vec::<Point>::new();
    let mut closed = false;
    let mut i = 0_usize;

    while i < segment.len() {
        let pair = &segment[i];
        match pair.code {
            73 => {
                if let Ok(flag) = pair.value.parse::<i32>() {
                    closed = flag == 1;
                }
                i += 1;
            }
            10 => {
                let Some(x) = parse_f64(&pair.value) else {
                    i += 1;
                    continue;
                };
                let mut y = None;
                let mut j = i + 1;
                while j < segment.len() {
                    if segment[j].code == 10 {
                        break;
                    }
                    if segment[j].code == 20 {
                        y = parse_f64(&segment[j].value);
                        break;
                    }
                    j += 1;
                }

                if let Some(y) = y {
                    vertices.push(Point { x, y, z: 0.0 });
                }
                i = j;
            }
            _ => i += 1,
        }
    }

    if closed
        && vertices.len() > 2
        && let Some(first) = vertices.first().copied()
    {
        vertices.push(first);
    }

    if vertices.len() >= 2 {
        Some(vertices)
    } else {
        None
    }
}

fn parse_hatch_edge_path(segment: &[Pair]) -> (Option<Vec<Point>>, bool) {
    let mut vertices = Vec::<Point>::new();
    let mut partial = false;
    let mut i = 0_usize;

    while i < segment.len() {
        if segment[i].code != 72 {
            i += 1;
            continue;
        }

        let edge_type = segment[i].value.parse::<i32>().unwrap_or(0);
        let start = i + 1;
        let mut end = start;
        while end < segment.len() && segment[end].code != 72 {
            end += 1;
        }
        let edge = &segment[start..end];

        match edge_type {
            1 => {
                if let Some((start_point, end_point)) = parse_hatch_line_edge(edge) {
                    append_chained_edge(&mut vertices, start_point, end_point);
                }
            }
            2 => {
                if let Some(arc_points) = parse_hatch_arc_edge(edge) {
                    append_segment_points(&mut vertices, &arc_points);
                } else {
                    partial = true;
                }
            }
            3 => {
                if let Some(ellipse_points) = parse_hatch_ellipse_edge(edge) {
                    append_segment_points(&mut vertices, &ellipse_points);
                } else {
                    partial = true;
                }
            }
            _ => {
                // Arc/ellipse/spline edge types are not yet modeled.
                partial = true;
            }
        }

        i = end;
    }

    if vertices.len() >= 2 {
        (Some(vertices), partial)
    } else {
        (None, partial)
    }
}

fn parse_hatch_line_edge(edge: &[Pair]) -> Option<(Point, Point)> {
    let mut x1 = None;
    let mut y1 = None;
    let mut x2 = None;
    let mut y2 = None;

    for pair in edge {
        match pair.code {
            10 => x1 = parse_f64(&pair.value),
            20 => y1 = parse_f64(&pair.value),
            11 => x2 = parse_f64(&pair.value),
            21 => y2 = parse_f64(&pair.value),
            _ => {}
        }
    }

    Some((
        Point {
            x: x1?,
            y: y1?,
            z: 0.0,
        },
        Point {
            x: x2?,
            y: y2?,
            z: 0.0,
        },
    ))
}

fn parse_hatch_arc_edge(edge: &[Pair]) -> Option<Vec<Point>> {
    let cx = value_for_code(edge, 10)?;
    let cy = value_for_code(edge, 20)?;
    let radius = value_for_code(edge, 40)?;
    let mut start_deg = value_for_code(edge, 50).unwrap_or(0.0);
    let mut end_deg = value_for_code(edge, 51).unwrap_or(360.0);
    let ccw = edge
        .iter()
        .find(|pair| pair.code == 73)
        .and_then(|pair| pair.value.parse::<i32>().ok())
        .map(|v| v != 0)
        .unwrap_or(true);

    if ccw {
        if end_deg <= start_deg {
            end_deg += 360.0;
        }
    } else {
        std::mem::swap(&mut start_deg, &mut end_deg);
        if end_deg <= start_deg {
            end_deg += 360.0;
        }
    }

    let span = (end_deg - start_deg).max(1.0);
    let segments = ((span / 360.0) * 48.0).ceil().clamp(8.0, 96.0) as usize;
    Some(sample_arc(
        Point {
            x: cx,
            y: cy,
            z: 0.0,
        },
        radius,
        start_deg,
        end_deg,
        segments,
    ))
}

fn parse_hatch_ellipse_edge(edge: &[Pair]) -> Option<Vec<Point>> {
    let cx = value_for_code(edge, 10)?;
    let cy = value_for_code(edge, 20)?;
    let major_x = value_for_code(edge, 11)?;
    let major_y = value_for_code(edge, 21)?;
    let ratio = value_for_code(edge, 40)?.abs();
    if ratio <= 1e-9 {
        return None;
    }
    let mut start_rad = value_for_code(edge, 50).unwrap_or(0.0).to_radians();
    let mut end_rad = value_for_code(edge, 51).unwrap_or(360.0).to_radians();
    let ccw = edge
        .iter()
        .find(|pair| pair.code == 73)
        .and_then(|pair| pair.value.parse::<i32>().ok())
        .map(|v| v != 0)
        .unwrap_or(true);

    let major_len = major_x.hypot(major_y);
    if major_len <= 1e-9 {
        return None;
    }

    let ux = major_x / major_len;
    let uy = major_y / major_len;
    let minor_x = -uy * major_len * ratio;
    let minor_y = ux * major_len * ratio;

    if ccw {
        if end_rad <= start_rad {
            end_rad += std::f64::consts::TAU;
        }
    } else {
        std::mem::swap(&mut start_rad, &mut end_rad);
        if end_rad <= start_rad {
            end_rad += std::f64::consts::TAU;
        }
    }

    let span = (end_rad - start_rad).max(0.01);
    let segments = ((span / std::f64::consts::TAU) * 96.0)
        .ceil()
        .clamp(12.0, 192.0) as usize;

    Some(sample_ellipse(
        Point {
            x: cx,
            y: cy,
            z: 0.0,
        },
        major_x,
        major_y,
        minor_x,
        minor_y,
        start_rad,
        end_rad,
        segments,
    ))
}

fn append_chained_edge(vertices: &mut Vec<Point>, start: Point, end: Point) {
    if let Some(last) = vertices.last().copied() {
        let same_start = (last.x - start.x).abs() < 1e-6 && (last.y - start.y).abs() < 1e-6;
        if !same_start {
            vertices.push(start);
        }
    } else {
        vertices.push(start);
    }
    vertices.push(end);
}

fn append_segment_points(vertices: &mut Vec<Point>, segment: &[Point]) {
    if segment.is_empty() {
        return;
    }
    for point in segment {
        if let Some(last) = vertices.last()
            && points_approx_equal(*last, *point)
        {
            continue;
        }
        vertices.push(*point);
    }
}

fn parse_insert(
    payload: &[Pair],
    blocks: &BTreeMap<String, BlockDefinition>,
) -> (Option<Vec<Entity>>, Option<String>) {
    let mut block_name = None;
    let mut insert_x = Some(0.0);
    let mut insert_y = Some(0.0);
    let mut insert_z = Some(0.0);
    let mut scale_x = Some(1.0);
    let mut scale_y = Some(1.0);
    let mut scale_z = Some(1.0);
    let mut rotation_deg = Some(0.0);
    let mut cols = Some(1_i32);
    let mut rows = Some(1_i32);
    let mut col_spacing = Some(0.0);
    let mut row_spacing = Some(0.0);

    for pair in payload {
        match pair.code {
            2 => block_name = Some(pair.value.clone()),
            10 => insert_x = parse_f64(&pair.value),
            20 => insert_y = parse_f64(&pair.value),
            30 => insert_z = parse_f64(&pair.value),
            41 => scale_x = parse_f64(&pair.value),
            42 => scale_y = parse_f64(&pair.value),
            43 => scale_z = parse_f64(&pair.value),
            50 => rotation_deg = parse_f64(&pair.value),
            70 => cols = pair.value.parse::<i32>().ok(),
            71 => rows = pair.value.parse::<i32>().ok(),
            44 => col_spacing = parse_f64(&pair.value),
            45 => row_spacing = parse_f64(&pair.value),
            _ => {}
        }
    }

    let block_name = match block_name {
        Some(name) => name,
        None => {
            return (
                None,
                Some("INSERT is missing block name (code 2).".to_string()),
            );
        }
    };
    let Some(block) = blocks.get(&block_name) else {
        return (
            None,
            Some(format!("Unknown block reference in INSERT: {block_name}")),
        );
    };

    let base_insert = Point {
        x: insert_x.unwrap_or(0.0),
        y: insert_y.unwrap_or(0.0),
        z: insert_z.unwrap_or(0.0),
    };
    let sx = scale_x.unwrap_or(1.0);
    let sy = scale_y.unwrap_or(1.0);
    let sz = scale_z.unwrap_or(1.0);
    let rotation = rotation_deg.unwrap_or(0.0).to_radians();
    let cos_t = rotation.cos();
    let sin_t = rotation.sin();
    let col_count = cols.unwrap_or(1).max(1);
    let row_count = rows.unwrap_or(1).max(1);
    let col_step = col_spacing.unwrap_or(0.0);
    let row_step = row_spacing.unwrap_or(0.0);

    let expanded = expand_block_entities(
        block,
        base_insert,
        sx,
        sy,
        sz,
        cos_t,
        sin_t,
        row_count,
        col_count,
        row_step,
        col_step,
    );

    let mut warning = None;
    if row_count > 1 || col_count > 1 {
        warning = Some(format!(
            "INSERT array expanded for block {block_name}: {row_count} row(s) x {col_count} column(s)."
        ));
    }

    (Some(expanded), warning)
}

fn parse_dimension(
    payload: &[Pair],
    blocks: &BTreeMap<String, BlockDefinition>,
) -> (Option<Vec<Entity>>, Option<String>) {
    let layer = payload
        .iter()
        .find(|pair| pair.code == 8)
        .map(|pair| pair.value.clone());
    let block_name = payload
        .iter()
        .find(|pair| pair.code == 2)
        .map(|pair| pair.value.clone())
        .filter(|value| !value.is_empty());

    let insert_point = Point {
        x: value_for_code(payload, 10).unwrap_or(0.0),
        y: value_for_code(payload, 20).unwrap_or(0.0),
        z: value_for_code(payload, 30).unwrap_or(0.0),
    };
    let sx = value_for_code(payload, 41).unwrap_or(1.0);
    let sy = value_for_code(payload, 42).unwrap_or(sx);
    let sz = value_for_code(payload, 43).unwrap_or(1.0);
    let rotation = value_for_code(payload, 50).unwrap_or(0.0).to_radians();
    let cos_t = rotation.cos();
    let sin_t = rotation.sin();

    let mut warning = None;
    if let Some(block_name) = block_name
        && let Some(block) = blocks.get(&block_name)
    {
        let expanded = expand_block_entities(
            block,
            insert_point,
            sx,
            sy,
            sz,
            cos_t,
            sin_t,
            1,
            1,
            0.0,
            0.0,
        );
        if !expanded.is_empty() {
            return (Some(expanded), None);
        }
        warning = Some(format!(
            "DIMENSION block {block_name} did not contain renderable entities; using fallback geometry."
        ));
    } else if let Some(block_name) = payload
        .iter()
        .find(|pair| pair.code == 2)
        .map(|pair| pair.value.clone())
        .filter(|value| !value.is_empty())
    {
        warning = Some(format!(
            "Unknown block reference in DIMENSION: {block_name}; using fallback geometry."
        ));
    }

    let p1 = point_for_codes(payload, 13, 23, 33);
    let p2 = point_for_codes(payload, 14, 24, 34);

    let (Some(p1), Some(p2)) = (p1, p2) else {
        return (None, warning);
    };

    let mut entities = Vec::<Entity>::new();
    let vx = p2.x - p1.x;
    let vy = p2.y - p1.y;
    let vz = p2.z - p1.z;
    let measured = (vx * vx + vy * vy + vz * vz).sqrt();
    if measured <= 1e-9 {
        return (None, warning);
    }
    let ux = vx / measured;
    let uy = vy / measured;
    let px = -uy;
    let py = ux;

    entities.push(Entity {
        kind: "line",
        vertices: vec![p1, p2],
        layer: layer.clone(),
        text: None,
        text_height: None,
    });

    let explicit_text = payload
        .iter()
        .find(|pair| pair.code == 1)
        .map(|pair| pair.value.trim().to_string())
        .filter(|value| !value.is_empty());
    let dim_text = match explicit_text.as_deref() {
        None | Some("<>") => format!("{measured:.3}"),
        Some(raw) => normalize_text(raw),
    };
    let text_height = value_for_code(payload, 140)
        .or_else(|| value_for_code(payload, 40))
        .unwrap_or(2.5);
    let arrow_size = (measured * 0.08).clamp(text_height * 0.35, text_height * 1.25);

    for (tip, inward) in [(p1, 1.0_f64), (p2, -1.0_f64)] {
        let back_x = tip.x + inward * ux * arrow_size;
        let back_y = tip.y + inward * uy * arrow_size;
        let wing = arrow_size * 0.45;
        let wing_a = Point {
            x: back_x + px * wing,
            y: back_y + py * wing,
            z: tip.z,
        };
        let wing_b = Point {
            x: back_x - px * wing,
            y: back_y - py * wing,
            z: tip.z,
        };
        entities.push(Entity {
            kind: "line",
            vertices: vec![tip, wing_a],
            layer: layer.clone(),
            text: None,
            text_height: None,
        });
        entities.push(Entity {
            kind: "line",
            vertices: vec![tip, wing_b],
            layer: layer.clone(),
            text: None,
            text_height: None,
        });
    }

    let text_anchor = point_for_codes(payload, 11, 21, 31).unwrap_or(Point {
        x: (p1.x + p2.x) * 0.5 + px * arrow_size * 1.2,
        y: (p1.y + p2.y) * 0.5 + py * arrow_size * 1.2,
        z: (p1.z + p2.z) * 0.5,
    });
    if let Some(text_entity) = build_text_entity(
        text_anchor.x,
        text_anchor.y,
        text_anchor.z,
        text_height,
        layer,
        dim_text,
    ) {
        entities.push(text_entity);
    }

    (Some(entities), warning)
}

fn parse_xline(payload: &[Pair]) -> Option<Entity> {
    parse_xline_or_ray(payload, false)
}

fn parse_ray(payload: &[Pair]) -> Option<Entity> {
    parse_xline_or_ray(payload, true)
}

fn parse_xline_or_ray(payload: &[Pair], ray: bool) -> Option<Entity> {
    let layer = payload
        .iter()
        .find(|pair| pair.code == 8)
        .map(|pair| pair.value.clone());
    let base = Point {
        x: value_for_code(payload, 10)?,
        y: value_for_code(payload, 20)?,
        z: value_for_code(payload, 30).unwrap_or(0.0),
    };
    let dir = Point {
        x: value_for_code(payload, 11)?,
        y: value_for_code(payload, 21)?,
        z: value_for_code(payload, 31).unwrap_or(0.0),
    };

    let length = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
    if length <= 1e-9 {
        return None;
    }

    let ux = dir.x / length;
    let uy = dir.y / length;
    let uz = dir.z / length;
    let extent = 1_000.0;
    let start = if ray {
        base
    } else {
        Point {
            x: base.x - ux * extent,
            y: base.y - uy * extent,
            z: base.z - uz * extent,
        }
    };
    let end = Point {
        x: base.x + ux * extent,
        y: base.y + uy * extent,
        z: base.z + uz * extent,
    };

    Some(Entity {
        kind: "line",
        vertices: vec![start, end],
        layer,
        text: None,
        text_height: None,
    })
}

fn parse_leader(payload: &[Pair]) -> Option<Vec<Entity>> {
    parse_leader_like(payload, &[1])
}

fn parse_multileader(payload: &[Pair]) -> Option<Vec<Entity>> {
    let layer = payload
        .iter()
        .find(|pair| pair.code == 8)
        .map(|pair| pair.value.clone());
    let points = collect_point_sequence(payload, 10, 20, 30);
    let text = collect_text_chunks_ordered(payload, &[304, 302, 303, 1]);
    let text_height = value_for_code(payload, 41)
        .or_else(|| value_for_code(payload, 40))
        .or_else(|| value_for_code(payload, 45))
        .unwrap_or(2.5);
    let anchor = point_for_codes(payload, 11, 21, 31)
        .or_else(|| points.last().copied())
        .or_else(|| point_for_codes(payload, 10, 20, 30));

    let mut entities = Vec::<Entity>::new();
    if points.len() >= 2 {
        entities.push(Entity {
            kind: "polyline",
            vertices: points,
            layer: layer.clone(),
            text: None,
            text_height: None,
        });
    }

    if let (Some(text), Some(anchor)) = (text, anchor)
        && let Some(text_entity) =
            build_text_entity(anchor.x, anchor.y, anchor.z, text_height, layer, text)
    {
        entities.push(text_entity);
    }

    if entities.is_empty() {
        None
    } else {
        Some(entities)
    }
}

fn parse_leader_like(payload: &[Pair], text_codes: &[i32]) -> Option<Vec<Entity>> {
    let layer = payload
        .iter()
        .find(|pair| pair.code == 8)
        .map(|pair| pair.value.clone());
    let points = collect_point_sequence(payload, 10, 20, 30);
    if points.len() < 2 {
        return None;
    }

    let mut entities = vec![Entity {
        kind: "polyline",
        vertices: points.clone(),
        layer: layer.clone(),
        text: None,
        text_height: None,
    }];

    let text = first_non_empty_text(payload, text_codes);
    if let Some(text) = text {
        let text_height = value_for_code(payload, 40).unwrap_or(2.5);
        let anchor = point_for_codes(payload, 11, 21, 31).unwrap_or_else(|| {
            points.last().copied().unwrap_or(Point {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            })
        });
        if let Some(text_entity) =
            build_text_entity(anchor.x, anchor.y, anchor.z, text_height, layer, text)
        {
            entities.push(text_entity);
        }
    }

    Some(entities)
}

fn first_non_empty_text(payload: &[Pair], codes: &[i32]) -> Option<String> {
    for code in codes {
        for pair in payload {
            if pair.code == *code {
                let cleaned = normalize_text(&pair.value);
                if !cleaned.is_empty() {
                    return Some(cleaned);
                }
            }
        }
    }
    None
}

fn collect_text_chunks_ordered(payload: &[Pair], preferred_codes: &[i32]) -> Option<String> {
    let mut chunks = Vec::<String>::new();
    for pair in payload {
        if preferred_codes.contains(&pair.code) {
            let cleaned = normalize_text(&pair.value);
            if !cleaned.is_empty() {
                chunks.push(cleaned);
            }
        }
    }
    if chunks.is_empty() {
        return None;
    }
    Some(chunks.join(" "))
}

fn expand_block_entities(
    block: &BlockDefinition,
    base_insert: Point,
    sx: f64,
    sy: f64,
    sz: f64,
    cos_t: f64,
    sin_t: f64,
    row_count: i32,
    col_count: i32,
    row_step: f64,
    col_step: f64,
) -> Vec<Entity> {
    let mut expanded = Vec::<Entity>::new();
    for row in 0..row_count {
        for col in 0..col_count {
            let offset_local = Point {
                x: col as f64 * col_step,
                y: row as f64 * row_step,
                z: 0.0,
            };
            let offset_world = rotate_and_scale(offset_local, sx, sy, 1.0, cos_t, sin_t);
            let insert_point = Point {
                x: base_insert.x + offset_world.x,
                y: base_insert.y + offset_world.y,
                z: base_insert.z,
            };

            for entity in &block.entities {
                expanded.push(transform_entity(
                    entity,
                    block.base,
                    insert_point,
                    sx,
                    sy,
                    sz,
                    cos_t,
                    sin_t,
                ));
            }
        }
    }
    expanded
}

fn value_for_code(payload: &[Pair], code: i32) -> Option<f64> {
    payload
        .iter()
        .find(|pair| pair.code == code)
        .and_then(|pair| parse_f64(&pair.value))
}

fn point_for_codes(payload: &[Pair], x_code: i32, y_code: i32, z_code: i32) -> Option<Point> {
    Some(Point {
        x: value_for_code(payload, x_code)?,
        y: value_for_code(payload, y_code)?,
        z: value_for_code(payload, z_code).unwrap_or(0.0),
    })
}

fn points_approx_equal(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6 && (a.z - b.z).abs() < 1e-6
}

fn transform_entity(
    entity: &Entity,
    block_base: Point,
    insert_point: Point,
    sx: f64,
    sy: f64,
    sz: f64,
    cos_t: f64,
    sin_t: f64,
) -> Entity {
    let mut transformed = entity.clone();
    transformed.vertices = entity
        .vertices
        .iter()
        .map(|point| {
            let local = Point {
                x: point.x - block_base.x,
                y: point.y - block_base.y,
                z: point.z - block_base.z,
            };
            let mapped = rotate_and_scale(local, sx, sy, sz, cos_t, sin_t);
            Point {
                x: insert_point.x + mapped.x,
                y: insert_point.y + mapped.y,
                z: insert_point.z + mapped.z,
            }
        })
        .collect();

    if let Some(height) = transformed.text_height {
        let average_scale = ((sx.abs() + sy.abs()) / 2.0).max(0.01);
        transformed.text_height = Some(height * average_scale);
    }

    transformed
}

fn rotate_and_scale(point: Point, sx: f64, sy: f64, sz: f64, cos_t: f64, sin_t: f64) -> Point {
    let scaled_x = point.x * sx;
    let scaled_y = point.y * sy;
    Point {
        x: scaled_x * cos_t - scaled_y * sin_t,
        y: scaled_x * sin_t + scaled_y * cos_t,
        z: point.z * sz,
    }
}

fn parse_text(payload: &[Pair]) -> Option<Entity> {
    let mut x = None;
    let mut y = None;
    let mut z = Some(0.0);
    let mut height = None;
    let mut layer = None;
    let mut text = None;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => x = parse_f64(&pair.value),
            20 => y = parse_f64(&pair.value),
            30 => z = parse_f64(&pair.value),
            40 => height = parse_f64(&pair.value),
            1 => text = Some(pair.value.clone()),
            _ => {}
        }
    }

    build_text_entity(
        x?,
        y?,
        z.unwrap_or(0.0),
        height.unwrap_or(2.5),
        layer,
        text?,
    )
}

fn parse_attribute_text(payload: &[Pair]) -> Option<Entity> {
    let mut x = None;
    let mut y = None;
    let mut z = Some(0.0);
    let mut height = None;
    let mut layer = None;
    let mut value = None;
    let mut tag = None;

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => x = parse_f64(&pair.value),
            20 => y = parse_f64(&pair.value),
            30 => z = parse_f64(&pair.value),
            40 => height = parse_f64(&pair.value),
            1 => value = Some(pair.value.clone()),
            2 => tag = Some(pair.value.clone()),
            _ => {}
        }
    }

    let text = value
        .or(tag)
        .map(|s| normalize_text(&s))
        .filter(|s| !s.is_empty())?;

    build_text_entity(x?, y?, z.unwrap_or(0.0), height.unwrap_or(2.5), layer, text)
}

fn parse_mtext(payload: &[Pair]) -> Option<Entity> {
    let mut x = None;
    let mut y = None;
    let mut z = Some(0.0);
    let mut height = None;
    let mut layer = None;
    let mut chunks = Vec::<String>::new();

    for pair in payload {
        match pair.code {
            8 => layer = Some(pair.value.clone()),
            10 => x = parse_f64(&pair.value),
            20 => y = parse_f64(&pair.value),
            30 => z = parse_f64(&pair.value),
            40 => height = parse_f64(&pair.value),
            3 | 1 => chunks.push(pair.value.clone()),
            _ => {}
        }
    }

    if chunks.is_empty() {
        return None;
    }

    build_text_entity(
        x?,
        y?,
        z.unwrap_or(0.0),
        height.unwrap_or(2.5),
        layer,
        chunks.join(""),
    )
}

fn build_text_entity(
    x: f64,
    y: f64,
    z: f64,
    raw_height: f64,
    layer: Option<String>,
    raw_text: String,
) -> Option<Entity> {
    let height = if raw_height.is_finite() && raw_height > 0.0 {
        raw_height
    } else {
        2.5
    };

    let text = normalize_text(&raw_text);
    if text.is_empty() {
        return None;
    }

    let width_estimate = (text.chars().count().max(1) as f64) * height * 0.6;
    let vertices = vec![
        Point { x, y, z },
        Point {
            x: x + width_estimate,
            y: y + height,
            z,
        },
    ];

    Some(Entity {
        kind: "text",
        vertices,
        layer,
        text: Some(text),
        text_height: Some(height),
    })
}

fn normalize_text(value: &str) -> String {
    value
        .replace("\\\\P", " ")
        .replace("\\\\p", " ")
        .replace("\\\\~", " ")
        .replace('{', "")
        .replace('}', "")
        .trim()
        .to_string()
}

fn collect_point_sequence(payload: &[Pair], x_code: i32, y_code: i32, z_code: i32) -> Vec<Point> {
    let mut points = Vec::<Point>::new();

    for (index, pair) in payload.iter().enumerate() {
        if pair.code != x_code {
            continue;
        }

        let x = match parse_f64(&pair.value) {
            Some(v) => v,
            None => continue,
        };

        let mut y = None;
        let mut z = 0.0;

        for next in &payload[(index + 1)..] {
            if next.code == x_code {
                break;
            }
            if next.code == y_code {
                y = parse_f64(&next.value);
            } else if next.code == z_code
                && let Some(value) = parse_f64(&next.value)
            {
                z = value;
            }
        }

        if let Some(y) = y {
            points.push(Point { x, y, z });
        }
    }

    points
}

fn sample_ellipse(
    center: Point,
    major_x: f64,
    major_y: f64,
    minor_x: f64,
    minor_y: f64,
    start_param: f64,
    end_param: f64,
    segments: usize,
) -> Vec<Point> {
    let mut points = Vec::<Point>::with_capacity(segments + 1);
    let total = (end_param - start_param).max(0.01);

    for step in 0..=segments {
        let t = step as f64 / segments as f64;
        let angle = start_param + total * t;
        points.push(Point {
            x: center.x + major_x * angle.cos() + minor_x * angle.sin(),
            y: center.y + major_y * angle.cos() + minor_y * angle.sin(),
            z: center.z,
        });
    }

    points
}

fn sample_arc(
    center: Point,
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    segments: usize,
) -> Vec<Point> {
    let mut points = Vec::<Point>::with_capacity(segments + 1);
    let total = (end_deg - start_deg).max(1.0);

    for step in 0..=segments {
        let t = step as f64 / segments as f64;
        let angle = (start_deg + total * t).to_radians();
        points.push(Point {
            x: center.x + radius * angle.cos(),
            y: center.y + radius * angle.sin(),
            z: center.z,
        });
    }

    points
}

fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

fn success_json(entities: &[Entity], warnings: &[String]) -> String {
    let mut out = String::from("{\"ok\":true,\"entities\":[");

    for (index, entity) in entities.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }

        out.push_str("{\"kind\":\"");
        out.push_str(entity.kind);
        out.push_str("\",");

        out.push_str("\"layer\":");
        match &entity.layer {
            Some(layer) => {
                out.push('"');
                out.push_str(&json_escape(layer));
                out.push('"');
            }
            None => out.push_str("null"),
        }
        out.push(',');

        out.push_str("\"text\":");
        match &entity.text {
            Some(text) => {
                out.push('"');
                out.push_str(&json_escape(text));
                out.push('"');
            }
            None => out.push_str("null"),
        }
        out.push(',');

        out.push_str("\"textHeight\":");
        match entity.text_height {
            Some(text_height) => {
                let _ = write!(out, "{:.6}", text_height);
            }
            None => out.push_str("null"),
        }
        out.push(',');

        out.push_str("\"vertices\":[");
        for (point_index, point) in entity.vertices.iter().enumerate() {
            if point_index > 0 {
                out.push(',');
            }
            let _ = write!(out, "[{:.6},{:.6},{:.6}]", point.x, point.y, point.z);
        }
        out.push_str("]}");
    }

    out.push_str("],\"warnings\":[");

    for (index, warning) in warnings.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&json_escape(warning));
        out.push('"');
    }

    out.push_str("]}");
    out
}

fn error_json(message: &str) -> String {
    format!(
        "{{\"ok\":false,\"error\":\"{}\",\"warnings\":[]}}",
        json_escape(message)
    )
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", c as u32);
            }
            c => escaped.push(c),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_line() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n20\n21\n10\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "line");
        assert!(warnings.is_empty());
    }

    #[test]
    fn reports_empty_entities_as_warning() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
        let (_, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("No renderable entities"));
    }

    #[test]
    fn parses_polyline_with_vertex_sequence() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nPOLYLINE\n8\nCENTER\n70\n1\n0\nVERTEX\n10\n0\n20\n0\n30\n0\n0\nVERTEX\n10\n10\n20\n0\n30\n0\n0\nVERTEX\n10\n10\n20\n5\n30\n0\n0\nSEQEND\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "polyline");
        // Closed flag repeats first vertex at the end.
        assert_eq!(entities[0].vertices.len(), 4);
        assert!(warnings.is_empty());
    }

    #[test]
    fn malformed_polyline_is_skipped_gracefully() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nPOLYLINE\n70\n1\n0\nVERTEX\n10\n0\n20\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert!(entities.is_empty());
        assert!(!warnings.is_empty());
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Skipped malformed POLYLINE"))
        );
    }

    #[test]
    fn polyline_skips_polyface_face_records() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nPOLYLINE\n70\n64\n0\nVERTEX\n70\n64\n10\n0\n20\n0\n30\n0\n0\nVERTEX\n70\n64\n10\n10\n20\n0\n30\n0\n0\nVERTEX\n70\n128\n71\n1\n72\n2\n0\nSEQEND\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].vertices.len(), 2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_point_as_visible_marker_line() {
        let dxf =
            "0\nSECTION\n2\nENTITIES\n0\nPOINT\n8\nPNT\n10\n5\n20\n6\n30\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "line");
        assert_eq!(entities[0].vertices.len(), 2);
        assert!((entities[0].vertices[0].y - 6.0).abs() < 0.001);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_3dface_as_wireframe_outline() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\n3DFACE\n8\nFACE\n10\n0\n20\n0\n30\n0\n11\n4\n21\n0\n31\n0\n12\n4\n22\n3\n32\n0\n13\n0\n23\n3\n33\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "polyline");
        assert_eq!(entities[0].vertices.len(), 5);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_solid_and_trace_as_outlines() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nSOLID\n8\nS\n10\n0\n20\n0\n30\n0\n11\n4\n21\n0\n31\n0\n12\n4\n22\n2\n32\n0\n13\n0\n23\n2\n33\n0\n0\nTRACE\n8\nT\n10\n5\n20\n0\n30\n0\n11\n7\n21\n0\n31\n0\n12\n7\n22\n2\n32\n0\n13\n5\n23\n2\n33\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 2);
        assert!(entities.iter().all(|e| e.kind == "polyline"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_text_entity() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nTEXT\n8\nANNO\n10\n4\n20\n5\n30\n0\n40\n3\n1\nHELLO\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "text");
        assert_eq!(entities[0].text.as_deref(), Some("HELLO"));
        assert_eq!(entities[0].vertices.len(), 2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_mtext_entity_and_normalizes_content() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nMTEXT\n8\nANNO\n10\n0\n20\n0\n30\n0\n40\n2.5\n3\nLINE1\\\\P\n1\nLINE2\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "text");
        assert_eq!(entities[0].text.as_deref(), Some("LINE1 LINE2"));
        assert_eq!(entities[0].text_height, Some(2.5));
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_attdef_and_attrib_as_text() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nATTDEF\n8\nANNO\n10\n0\n20\n0\n30\n0\n40\n2.5\n1\nDEFAULT\n2\nTAG_A\n0\nATTRIB\n8\nANNO\n10\n3\n20\n0\n30\n0\n40\n2.5\n1\nVAL1\n2\nTAG_A\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 2);
        assert!(entities.iter().all(|e| e.kind == "text"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_full_ellipse_entity() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nELLIPSE\n8\nROUND\n10\n0\n20\n0\n30\n0\n11\n10\n21\n0\n31\n0\n40\n0.5\n41\n0\n42\n6.283185307179586\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "polyline");
        assert!(entities[0].vertices.len() >= 90);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_ellipse_arc_entity() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nELLIPSE\n8\nROUND\n10\n0\n20\n0\n30\n0\n11\n10\n21\n0\n31\n0\n40\n0.5\n41\n0\n42\n1.5707963267948966\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        let first = entities[0].vertices.first().expect("first");
        let last = entities[0].vertices.last().expect("last");
        assert!((first.x - 10.0).abs() < 0.1);
        assert!(first.y.abs() < 0.1);
        assert!(last.x.abs() < 0.2);
        assert!((last.y - 5.0).abs() < 0.2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_spline_from_control_points() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nSPLINE\n8\nCURVE\n70\n0\n10\n0\n20\n0\n30\n0\n10\n10\n20\n5\n30\n0\n10\n20\n20\n0\n30\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "polyline");
        assert_eq!(entities[0].vertices.len(), 3);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_spline_from_fit_points() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nSPLINE\n8\nCURVE\n70\n0\n11\n0\n21\n0\n31\n0\n11\n5\n21\n10\n31\n0\n11\n10\n21\n0\n31\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].vertices.len(), 3);
        assert!((entities[0].vertices[1].x - 5.0).abs() < 0.001);
        assert!((entities[0].vertices[1].y - 10.0).abs() < 0.001);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_hatch_polyline_boundary() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nHATCH\n8\nFILL\n10\n0\n20\n0\n30\n0\n70\n1\n71\n0\n91\n1\n92\n3\n72\n0\n73\n1\n93\n4\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n5\n10\n0\n20\n5\n97\n0\n75\n0\n76\n1\n98\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "polyline");
        assert_eq!(entities[0].vertices.len(), 5);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_hatch_edge_path_with_line_edges() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nHATCH\n8\nFILL\n10\n0\n20\n0\n30\n0\n70\n1\n71\n0\n91\n1\n92\n1\n93\n4\n72\n1\n10\n0\n20\n0\n11\n8\n21\n0\n72\n1\n10\n8\n20\n0\n11\n8\n21\n4\n72\n1\n10\n8\n20\n4\n11\n0\n21\n4\n72\n1\n10\n0\n20\n4\n11\n0\n21\n0\n75\n0\n76\n1\n98\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "polyline");
        assert!(entities[0].vertices.len() >= 5);
        assert!(warnings.is_empty());
    }

    #[test]
    fn parses_hatch_edge_path_with_arc_and_ellipse_edges() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nHATCH\n8\nFILL\n10\n0\n20\n0\n30\n0\n70\n1\n71\n0\n91\n1\n92\n1\n93\n2\n72\n2\n10\n0\n20\n0\n40\n5\n50\n0\n51\n180\n73\n1\n72\n3\n10\n0\n20\n0\n11\n6\n21\n0\n40\n0.5\n50\n180\n51\n360\n73\n1\n75\n0\n76\n1\n98\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "polyline");
        assert!(entities[0].vertices.len() > 20);
        assert!(warnings.is_empty());
    }

    #[test]
    fn expands_insert_from_block_definition() {
        let dxf = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n8\n0\n2\nB1\n10\n0\n20\n0\n30\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n5\n21\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n8\n0\n2\nB1\n10\n20\n20\n30\n30\n0\n41\n2\n42\n2\n50\n90\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "line");
        let first = entities[0].vertices.first().expect("first");
        let last = entities[0].vertices.last().expect("last");
        assert!((first.x - 20.0).abs() < 0.01);
        assert!((first.y - 30.0).abs() < 0.01);
        assert!((last.x - 20.0).abs() < 0.05);
        assert!((last.y - 40.0).abs() < 0.05);
        assert!(warnings.is_empty());
    }

    #[test]
    fn insert_unknown_block_reports_warning() {
        let dxf =
            "0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nMISSING\n10\n0\n20\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert!(entities.is_empty());
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Unknown block reference"))
        );
    }

    #[test]
    fn dimension_fallback_renders_line_and_text() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nDIMENSION\n8\nANNO\n70\n0\n13\n0\n23\n0\n14\n12\n24\n0\n11\n6\n21\n2\n1\n<>\n140\n2.5\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert!(entities.len() >= 6);
        assert_eq!(entities[0].kind, "line");
        assert!(
            entities
                .iter()
                .any(|e| e.kind == "text" && e.text.as_deref() == Some("12.000"))
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn dimension_expands_block_when_available() {
        let dxf = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n8\n0\n2\n*D1\n10\n0\n20\n0\n30\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nDIMENSION\n8\nANNO\n2\n*D1\n10\n40\n20\n50\n30\n0\n41\n1.5\n42\n1.5\n50\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "line");
        let first = entities[0].vertices.first().expect("first");
        let last = entities[0].vertices.last().expect("last");
        assert!((first.x - 40.0).abs() < 0.01);
        assert!((first.y - 50.0).abs() < 0.01);
        assert!((last.x - 55.0).abs() < 0.05);
        assert!((last.y - 50.0).abs() < 0.05);
        assert!(warnings.is_empty());
    }

    #[test]
    fn dimension_unknown_block_falls_back_with_warning() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nDIMENSION\n2\nMISSING\n13\n0\n23\n0\n14\n10\n24\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert!(entities.len() >= 5);
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Unknown block reference in DIMENSION"))
        );
    }

    #[test]
    fn leader_fallback_renders_polyline_and_text() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLEADER\n8\nANNO\n10\n0\n20\n0\n30\n0\n10\n4\n20\n3\n30\n0\n10\n9\n20\n3\n30\n0\n1\nLEAD-NOTE\n40\n2.0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].kind, "polyline");
        assert_eq!(entities[1].kind, "text");
        assert_eq!(entities[1].text.as_deref(), Some("LEAD-NOTE"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn multileader_fallback_renders_polyline_and_text() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nMULTILEADER\n8\nANNO\n10\n20\n20\n0\n30\n0\n10\n24\n20\n3\n30\n0\n10\n28\n20\n3\n30\n0\n304\nML NOTE\n40\n2.2\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].kind, "polyline");
        assert_eq!(entities[1].kind, "text");
        assert_eq!(entities[1].text.as_deref(), Some("ML NOTE"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn multileader_collects_multiple_text_chunks() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nMULTILEADER\n8\nANNO\n10\n10\n20\n10\n30\n0\n10\n12\n20\n12\n30\n0\n304\nLINE1\n304\nLINE2\n302\nEXTRA\n40\n2.0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[1].kind, "text");
        assert!(entities[1].text.as_deref().unwrap_or("").contains("LINE1"));
        assert!(entities[1].text.as_deref().unwrap_or("").contains("LINE2"));
        assert!(entities[1].text.as_deref().unwrap_or("").contains("EXTRA"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn xline_fallback_renders_long_line() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nXLINE\n8\nAUX\n10\n0\n20\n0\n30\n0\n11\n1\n21\n0\n31\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "line");
        let first = entities[0].vertices.first().expect("first");
        let last = entities[0].vertices.last().expect("last");
        assert!(first.x < -900.0);
        assert!(last.x > 900.0);
        assert!(first.y.abs() < 0.001);
        assert!(last.y.abs() < 0.001);
        assert!(warnings.is_empty());
    }

    #[test]
    fn ray_fallback_renders_half_line() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nRAY\n8\nAUX\n10\n5\n20\n5\n30\n0\n11\n1\n21\n1\n31\n0\n0\nENDSEC\n0\nEOF\n";
        let (entities, warnings) = parse_entities_from_text(dxf).expect("should parse");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].kind, "line");
        let first = entities[0].vertices.first().expect("first");
        let last = entities[0].vertices.last().expect("last");
        assert!((first.x - 5.0).abs() < 0.001);
        assert!((first.y - 5.0).abs() < 0.001);
        assert!(last.x > 700.0);
        assert!(last.y > 700.0);
        assert!(warnings.is_empty());
    }
}
