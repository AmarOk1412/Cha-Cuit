// TODO license
// TODO i18n
// TODO docs
function farenheit_to_celsius(value) {
    return Math.round(parseInt(value) / 1.8 - 32)
}

function celsius_to_fahrenheit(value) {
    return Math.round((parseInt(value) + 32) * 1.8)
}

const INGREDIENT_DATA = [
    ["eau", "water", 1, true],
    ["beurre", "butter", .911, false],
    ["farine", "flour", .600, false],
    ["sucre", "sugar", .845, false],
    ["lait", "milk", 1.03, true],
    ["sel", "salt", 1.217, false],
    ["miel", "honey", 1.420, true],
    ["huile", "oil", .918, true],
    ["riz", "rice", .850, false],
    ["amandes", "oats", .410, false],
    ["cacao", "cacao", .520, false],
    ["poudre d'amande", "almond flour", 1.09, false],
    ["chocolat", "chocolate", .64, false],
    ["crème", "cream", 1, true]
];

function to_ingredient(string) {
    for (const [fr_ingredient, ingredient, density, isLiquid] of INGREDIENT_DATA) {
        if (string.includes(fr_ingredient)) {
            return ingredient;
        }
    }

    return "";
}

function ingredient_density(ingredient) {
    for (const [fr_ingredient, ingredientName, density, isLiquid] of INGREDIENT_DATA) {
        if (ingredientName === ingredient) {
            return density;
        }
    }
    return 1;
}

function is_liquid(ingredient) {
    for (const [fr_ingredient, ingredientName, density, isLiquid] of INGREDIENT_DATA) {
        if (ingredientName === ingredient) {
            return isLiquid;
        }
    }
    return false;
}

var cups_size = 250. // mL (Canadian size) - US = 236.588236
var prefCelsius = true
var prefCups = false

function grams_to_cups(value, ingredient) {
    return (parseFloat(value).toFixed(5) / (cups_size * ingredient_density(ingredient))).toFixed(3)
}

function cups_to_grams(value, ingredient) {
    return Math.round(parseFloat(value).toFixed(5) * cups_size * ingredient_density(ingredient))
}

// Canadian cup = 250mL
function liter_to_grams(value, ingredient) {
    return Math.round(parseFloat(value).toFixed(2) * 1000. * ingredient_density(ingredient))
}

function grams_to_liter(value, ingredient) {
    return Math.round(parseFloat(value).toFixed(2) / (1000. * ingredient_density(ingredient)))
}

// Canadian cup = 250mL
function cups_to_liter(value) {
    return parseFloat(value).toFixed(2) / 4
}

function liter_to_cups(value) {
    return parseFloat(value).toFixed(2) * 4
}

function to_user(value) {
    if (value > 0 && value < 0.15) {
        return "⅛"
    } else if (value >= 0.15 && value < 0.3) {
        return "¼"
    } else if (value >= 0.3 && value <= 0.4) {
        return "⅓"
    } else if (value >= 0.4 && value < 0.45) {
        return "⅖"
    } else if (value >= 0.45 && value < 0.55) {
        return "½"
    } else if (value >= 0.55 && value < 0.70) {
        return "⅝"
    } else if (value >= 0.70 && value < 0.85) {
        return "⅔"
    }
    return "⅞"
}

function to_unit_span(standard_value, non_standard_value, showStandard, unit_type) {
    var res = document.createElement("span")
    res.className = unit_type
    res.setAttribute("standard-text", standard_value)
    res.setAttribute("non-standard-text", non_standard_value)
    res.innerText = showStandard? standard_value : non_standard_value
    return res
}

function use_standard(span, useStandard) {
    if (useStandard) {
        span.innerText = span.getAttribute("standard-text")
    } else {
        span.innerText = span.getAttribute("non-standard-text")
    }
}

function update_spans() {
    var temperatureSpans = document.getElementsByClassName("temperature")
    for (var span of temperatureSpans) {
        use_standard(span, prefCelsius)
    }
    var quantitySpans = document.getElementsByClassName("quantity")
    for (var span of quantitySpans) {
        use_standard(span, !prefCups)
    }
}

function prepare_temperature_spans(body) {
    for (const match of body.matchAll(/([0-9]+)°F/g)) {
        farenheit_text = match[0]
        celsius_text = farenheit_to_celsius(match[1]) + "°C"
        var temp_span = to_unit_span(celsius_text, farenheit_text, prefCelsius, "temperature")
        body = body.replaceAll(match[0], temp_span.outerHTML)
    }
    for (const match of body.matchAll(/([0-9]+)°C/g)) {
        celsius_text = match[0]
        farenheit_text = celsius_to_fahrenheit(match[1]) + "°F"
        var temp_span = to_unit_span(celsius_text, farenheit_text, prefCelsius, "temperature")
        body = body.replaceAll(match[0], temp_span.outerHTML)
    }
    return body
}

function cups_to_string(cups) {
    if (cups % 1 > 0) {
        if (Math.floor(cups) === 0)
            return to_user(cups % 1) + " tasse(s) "
        else
            return Math.floor(cups) + " tasse(s) " + to_user(cups % 1) + " "
    }
    else
        return Math.floor(cups) + " tasse(s) "
}

function prepare_quantity_spans(body) {
    const matches = (body, regex, toString, ratio) => {
        for (const match of body.matchAll(regex)) {
            const ingredient = to_ingredient(match[2])
            if (ingredient === "")
                continue
            const span = toString(match, ingredient, ratio)
            body = body.replaceAll(match[0], span.outerHTML)
        }
        return body
    }

    var prev_body = body

    // Replace grams
    body = matches(body, /([0-9]+)g (([\w’ê]+\s*){1,4})/g, (match, ingredient) => {
        const cups =  grams_to_cups(match[1], ingredient)
        const cups_str = cups_to_string(cups)
        const cups_text = cups_str + match[2]
        return to_unit_span(match[0], cups_text, !prefCups, "quantity")
    })
    // Replace liquids
    const ratios_regex = [[1000, /([0-9]+)ml (([\w’ê]+\s*){1,4})/ig], [100, /([0-9]+)cl (([\w’ê]+\s*){1,4})/ig], [1, /([0-9]+)l (([\w’ê]+\s*){1,4})/ig]]
    for (const ratio_regex of ratios_regex) {
        body = matches(body, ratio_regex[1], (match, ingredient, ratio) => {
            const cups =  liter_to_cups(parseFloat(match[1])) / ratio
            const cups_str = cups_to_string(cups)
            const cups_text = cups_str + match[2]
            return to_unit_span(match[0], cups_text, !prefCups, "quantity")
        }, ratio_regex[0])
    }
    // Do not replace if already modified (to not pass again on same values)
    if (prev_body === body) {
        // Replace cups
        body = matches(body, /([0-9\.]+) tasse\(s\) (([\w’ê]+\s*){1,4})/g, (match, ingredient, ratio) => {
            var standard_text = ''
            if (is_liquid(ingredient)) {
                var unit = 'L'
                var quantity = cups_to_liter(match[1], ingredient)
                if (quantity < 0.01) {
                    unit = 'mL'
                    quantity *= 1000
                } else if (quantity < 1.0) {
                    unit = 'cL'
                    quantity *= 100
                }
                standard_text =  quantity + unit + " " + match[2]
            } else {
                standard_text = cups_to_grams(match[1], ingredient) + "g " + match[2]
            }

            return to_unit_span(standard_text, match[0], !prefCups, "quantity")
        })
    }

    return body
}

// TODO localisation
function parse_page() {
    var article = document.getElementsByClassName("prose")
    if (article.length === 0)
        return
    // If parsed once, it's easy to invert
    var parsedDiv = document.getElementsByClassName("temperature")
    var quantityDiv = document.getElementsByClassName("quantity")
    if (parsedDiv.length !== 0 || quantityDiv.length !== 0) {
        update_spans()
        return
    }
    // Else, regexes!
    var body = article[0].innerHTML

    body = prepare_temperature_spans(body)
    body = prepare_quantity_spans(body)

    article[0].innerHTML = body
}

// Store preferences

function setCookie(cname, cvalue, exdays=7) {
    const d = new Date()
    d.setTime(d.getTime() + (exdays * 24 * 60 * 60 * 1000))
    let expires = "expires="+d.toUTCString()
    document.cookie = cname + "=" + cvalue + ";" + expires + ";path=/;SameSite=Lax"
}

function getCookie(cname) {
    let name = cname + "="
    let ca = document.cookie.split(';')
    for (let i = 0; i < ca.length; i++) {
        let c = ca[i]
        while (c.charAt(0) === ' ') {
            c = c.substring(1)
        }
        if (c.indexOf(name) === 0) {
            return c.substring(name.length, c.length)
        }
    }
    return ""
}

function toggle_degrees() {
    prefCelsius = !prefCelsius
    setCookie('prefCelsius', prefCelsius)
}

function toggle_cups() {
    prefCups = !prefCups
    setCookie('prefCups', prefCups)
}

function get_preferences() {
    if (getCookie('prefCelsius') === '') {
        setCookie('prefCelsius', prefCelsius)
    }
    if (getCookie('prefCups') === '') {
        setCookie('prefCups', prefCups)
    }
    prefCelsius = getCookie('prefCelsius') === 'true'
    prefCups = getCookie('prefCups') === 'true'
}
