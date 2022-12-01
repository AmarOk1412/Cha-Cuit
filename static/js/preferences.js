// TODO license
// TODO improve
// TODO i18n
// TODO check on new content
function farenheit_to_celsius(value) {
    return Math.round(parseInt(value) / 1.8 - 32)
}

function celsius_to_fahrenheit(value) {
    return Math.round((parseInt(value) + 32) * 1.8)
}

// TODO group the 3 methods in an array
function ingredient_density(ingredient) {
    if (ingredient === "water") {
        return 1
    } else if (ingredient === "butter") {
        return .911
    } else if (ingredient === "flour") {
        return .600
    } else if (ingredient === "sugar") {
        return .845
    } else if (ingredient === "milk") {
        return 1.03
    } else if (ingredient === "salt") {
        return 1.217
    } else if (ingredient === "honey") {
        return 1.420
    } else if (ingredient === "oil") {
        return .918
    } else if (ingredient === "rice") {
        return .850
    } else if (ingredient === "oats") {
        return .410
    } else if (ingredient === "cacao") {
        return .520
    } else if (ingredient === "almond flour") {
        return 1.09
    } else if (ingredient === "chocolate") {
        return .64
    } else if (ingredient === "cream") {
        return 1
    }

    return 1
}

function is_liquid(ingredient) {
    if (ingredient === "water") {
        return true
    } else if (ingredient === "butter") {
        return false
    } else if (ingredient === "flour") {
        return false
    } else if (ingredient === "sugar") {
        return false
    } else if (ingredient === "milk") {
        return true
    } else if (ingredient === "salt") {
        return false
    } else if (ingredient === "honey") {
        return true
    } else if (ingredient === "oil") {
        return true
    } else if (ingredient === "rice") {
        return false
    } else if (ingredient === "oats") {
        return false
    } else if (ingredient === "cacao") {
        return false
    } else if (ingredient === "almond flour") {
        return false
    } else if (ingredient === "chocolate") {
        return false
    } else if (ingredient === "cream") {
        return true
    }

    return false
}

function to_ingredient(string) {
    if (string.includes("eau")) {
        return "water"
    } else if (string.includes("beurre")) {
        return "butter"
    } else if (string.includes("farine")) {
        return "flour"
    } else if (string.includes("sucre")) {
        return "sugar"
    } else if (string.includes("lait")) {
        return "milk"
    } else if (string.includes("sel")) {
        return "salt"
    } else if (string.includes("honey")) {
        return "honey"
    } else if (string.includes("huile")) {
        return "oil"
    } else if (string.includes("riz")) {
        return "rice"
    } else if (string.includes("avoine")) {
        return "oats"
    } else if (string.includes("cacao")) {
        return "cacao"
    } else if (string.includes("poudre d’amandes")) {
        return "almond flour"
    } else if (string.includes("chocolat")) {
        return "chocolate"
    } else if (string.includes("crême")) {
        return "cream"
    }

    return ""
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

// TODO localisation
function parse_page() {
    var article = document.getElementsByClassName("prose")
    var body = article[0].innerHTML

    if (prefCelsius) {
        for (const match of body.matchAll(/([0-9]+)°F/g)) {
            body = body.replaceAll(match[0], farenheit_to_celsius(match[1]) + "°C")
        }
    } else {
        for (const match of body.matchAll(/([0-9]+)°C/g)) {
            body = body.replaceAll(match[0], celsius_to_fahrenheit(match[1]) + "°F")
        }
    }

    if (prefCups) {
        for (const match of body.matchAll(/([0-9]+)g (\w+.\w+.\w+.\w+)/g)) {
            const ingredient = to_ingredient(match[2])
            if (ingredient === "")
                continue
            const newString = grams_to_cups(match[1], ingredient) + " tasse(s) " + match[2]
            body = body.replaceAll(match[0], newString)
        }
        for (const match of body.matchAll(/([0-9]+)ml (\w+.\w+.\w+.\w+)/ig)) {
            const ingredient = to_ingredient(match[2])
            if (ingredient === "")
                continue
            const newString = liter_to_cups(parseFloat(match[1])) / 1000 + " tasse(s) " + match[2]
            body = body.replaceAll(match[0], newString)
        }
        for (const match of body.matchAll(/([0-9]+)cl (\w+.\w+.\w+.\w+)/ig)) {
            const ingredient = to_ingredient(match[2])
            if (ingredient === "")
                continue
            const newString = liter_to_cups(parseFloat(match[1])) / 100 + " tasse(s) " + match[2]
            body = body.replaceAll(match[0], newString)
        }
        for (const match of body.matchAll(/([0-9]+)l (\w+.\w+.\w+.\w+)/ig)) {
            const ingredient = to_ingredient(match[2])
            if (ingredient === "")
                continue
            const newString = liter_to_cups(parseFloat(match[1])) + " tasse(s) " + match[2]
            body = body.replaceAll(match[0], newString)
        }
    } else {
        for (const match of body.matchAll(/([0-9\.]+) tasse\(s\) (\w+.\w+.\w+.\w+)/g)) {
            const ingredient = to_ingredient(match[2])
            if (ingredient === "")
                continue
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
                const newString =  quantity + unit + " " + match[2]
                body = body.replaceAll(match[0], newString)
            } else {
                const newString = cups_to_grams(match[1], ingredient) + "g " + match[2]
                body = body.replaceAll(match[0], newString)
            }
        }
    }

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
