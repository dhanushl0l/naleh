function openModal(product) {
    productName.textContent = product.firstname;
    productPrice.innerHTML = `₹ ${product.price}`;
    mainImage.src = product.images[0];

    imageGallery.innerHTML = "";
    product.images.forEach(img => {
        const imgElem = document.createElement("img");
        imgElem.src = img;
        imgElem.onclick = () => mainImage.src = img;
        imageGallery.appendChild(imgElem);
    });


    modal.style.display = "flex";

    discription.querySelectorAll("p").forEach(p => p.remove());

    const discreption_p = document.createElement("p");

    const formattedDescription = product.description
        .split('&')
        .map(part => {
            part = part.trim();
            return part.replace(/#(.*?)#/g, (_, match) => `<b>${match}</b>`);
        })
        .join('<br>');

    discreption_p.innerHTML = formattedDescription;

    discription.appendChild(discreption_p);

    info_box.querySelectorAll("p").forEach(p => p.remove());

    const info_box_type = document.createElement("p");

    const formattedDetails = product.details
        .split('&')
        .map(part => {
            part = part.trim();

            return part.replace(/#(.*?)#/g, (_, match) => `<b>${match}</b>`);
        })
        .join('<br>');

    info_box_type.innerHTML = formattedDetails;

    const url = new URL(window.location);
    url.searchParams.set("id", product.id);
    window.history.pushState({}, "", url);

    info_box.appendChild(info_box_type);
    button.onclick = () => {
        const url = product.url;
        window.open(url, "_blank");
    };

    document.addEventListener("keydown", function (e) {
        if (e.key === "Escape") {
            closeModal();
        }
    });
}

window.addEventListener('load', () => {
    document.querySelectorAll('img').forEach(img => {
        if (!img.complete || img.naturalWidth === 0) {
            const src = img.getAttribute('src');
            const newSrc = src.includes('?') ? `${src}&reload=${Date.now()}` : `${src}?reload=${Date.now()}`;
            img.setAttribute('src', newSrc);
        }
    });
});

function closeModal() {
    modal.style.display = "none";

    const url = new URL(window.location);
    url.searchParams.delete("id");
    window.history.replaceState({}, "", url);
}
